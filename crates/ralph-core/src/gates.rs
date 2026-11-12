//! Event gates: evidence an event must carry before it is routed.
//!
//! A gate (`gates:` in config) lists requirements for one event topic. Field
//! requirements read the event payload, using the same rules as autoloop's
//! `parseEvidenceEntry` (`packages/core/src/evidence.ts`): the payload is a
//! JSON object or a run of `key=value` tokens, and vacuous values count as
//! missing. Commit and command requirements are checked by Ralph itself.
//!
//! Requirements are evaluated in order and evaluation stops at the first
//! failure, so an expensive command never runs after a cheap field check has
//! already failed.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::LazyLock;

use regex::Regex;

use crate::config::{
    CommandRequirement, CommitVerify, FieldRequirement, GateConfig, GateRequirement,
};
use crate::hooks::{HookExecutor, HookExecutorContract, HookRunRequest};
use crate::text::{sanitize_for_process, truncate_with_ellipsis};

/// Timeout for a `command` requirement that sets none.
pub const DEFAULT_GATE_COMMAND_TIMEOUT_SECS: u64 = 300;

/// How much of a failing command's stderr a shortfall carries.
pub const GATE_STDERR_TAIL_BYTES: usize = 4 * 1024;

/// How much output a gate command may produce before capture stops. Capture
/// keeps the head, so this is generous to leave a meaningful stderr tail.
const GATE_COMMAND_MAX_OUTPUT_BYTES: u64 = 1024 * 1024;
/// Cap on `RALPH_EVENT_PAYLOAD`. Linux rejects a single env string over
/// `MAX_ARG_STRLEN` (128 KiB) at spawn, which would make the gate unpassable.
const GATE_EVENT_PAYLOAD_ENV_MAX_BYTES: usize = 64 * 1024;
/// Cap on `RALPH_EVENT_TOPIC`. Topics are short names; the cap only keeps an
/// absurd one from failing spawn or flooding the stdin pipe and details.
const GATE_EVENT_TOPIC_ENV_MAX_BYTES: usize = 1024;
/// Longest evidence value echoed back in a shortfall detail.
const DETAIL_VALUE_MAX_CHARS: usize = 120;
/// Longest commit value passed to git. No real revision is longer; anything
/// over this is not a commit, and an unbounded one would fail spawn (`E2BIG`).
const COMMIT_VALUE_MAX_BYTES: usize = 1024;

/// An evidence value as echoed in a shortfall detail: bounded, NULs visible.
fn shown(value: &str) -> String {
    truncate_with_ellipsis(value, DETAIL_VALUE_MAX_CHARS).replace('\0', "\\0")
}

const STATUS_WORDS: &[&str] = &["passed", "failed", "failing", "passing"];

static TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"([A-Za-z_][\w.-]*)=("[^"]*"|'[^']*'|[^\s,;]+)(?:\s+(passed|failed|failing|passing)\b)?"#,
    )
    .expect("token regex is valid")
});

// ─────────────────────────────────────────────────────────────────────────────
// Payload parsing
// ─────────────────────────────────────────────────────────────────────────────

/// One evidence key's parsed value from a payload.
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceEntry {
    /// Raw value text (for tokens, includes a trailing status word).
    pub raw: String,
    /// Leading number, if the value parses as one (a trailing `%` is allowed).
    pub numeric: Option<f64>,
    /// Status word, if one was given.
    pub status: Option<String>,
}

/// Extracts one evidence key from a payload.
///
/// Supports JSON object payloads (`{"k": "v"}` or the typed form
/// `{"k": {"value": "v", "status": "passed"}}`) and token payloads
/// (`k=v`, quoted or bare, optionally followed by a status word:
/// `k=5 passed`). Returns `None` when the key is absent or its value is
/// vacuous: an empty string, `false`, `null`, an array, or an object without
/// a `value`.
pub fn parse_evidence_entry(payload: &str, key: &str) -> Option<EvidenceEntry> {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('{')
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
    {
        return parse_json_entry(&value, key);
    }
    parse_token_entry(payload, key)
}

fn parse_json_entry(value: &serde_json::Value, key: &str) -> Option<EvidenceEntry> {
    use serde_json::Value;

    let entry = value.as_object()?.get(key)?;
    match entry {
        Value::Object(record) => {
            let raw = match record.get("value") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => return None,
            };
            if raw.trim().is_empty() {
                return None;
            }
            let status = record
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_string);
            Some(EvidenceEntry {
                numeric: parse_numeric(&raw),
                raw,
                status,
            })
        }
        Value::String(s) if !s.trim().is_empty() => Some(EvidenceEntry {
            numeric: parse_numeric(s),
            raw: s.clone(),
            status: None,
        }),
        Value::Number(n) => {
            let raw = n.to_string();
            Some(EvidenceEntry {
                numeric: parse_numeric(&raw),
                raw,
                status: None,
            })
        }
        Value::Bool(true) => Some(EvidenceEntry {
            raw: "true".to_string(),
            numeric: None,
            status: None,
        }),
        _ => None,
    }
}

fn parse_token_entry(payload: &str, key: &str) -> Option<EvidenceEntry> {
    let captures = TOKEN_RE.captures_iter(payload).find(|c| &c[1] == key)?;
    let value = &captures[2];
    let value = value.strip_prefix(['"', '\'']).unwrap_or(value);
    let value = value.strip_suffix(['"', '\'']).unwrap_or(value).trim();
    if value.is_empty() {
        return None;
    }
    let status = captures
        .get(3)
        .map(|m| m.as_str())
        .filter(|s| STATUS_WORDS.contains(s));
    let raw = match status {
        Some(status) => format!("{value} {status}"),
        None => value.to_string(),
    };
    Some(EvidenceEntry {
        raw,
        numeric: parse_numeric(value),
        status: status.map(str::to_string),
    })
}

/// Parses a number, stripping one trailing `%` (`"87%"` -> 87).
fn parse_numeric(raw: &str) -> Option<f64> {
    let cleaned = raw.trim();
    let cleaned = cleaned.strip_suffix('%').unwrap_or(cleaned).trim();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok().filter(|n| n.is_finite())
}

// ─────────────────────────────────────────────────────────────────────────────
// Shortfalls
// ─────────────────────────────────────────────────────────────────────────────

/// Why a requirement was not met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortfallReason {
    /// The field is absent or its value is vacuous.
    Missing,
    /// The field's status word is not the required one.
    Status,
    /// The field is not a number, or is outside `min`/`max`.
    Threshold,
    /// The field's value does not resolve to a commit.
    NotACommit,
    /// The commit is not reachable from the required ref.
    NotReachable,
    /// The commit was already reachable from HEAD at iteration start.
    NotNew,
    /// Git could not answer a commit check (not a repo, corrupt objects,
    /// bad iteration-start HEAD, git missing). Fails closed.
    GitFailed,
    /// The command exited with a non-zero code.
    CommandExited(i32),
    /// The command hit its timeout.
    CommandTimedOut,
    /// The command could not be run, or was killed by a signal.
    CommandFailed,
}

impl fmt::Display for ShortfallReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShortfallReason::Missing => f.write_str("missing"),
            ShortfallReason::Status => f.write_str("status"),
            ShortfallReason::Threshold => f.write_str("threshold"),
            ShortfallReason::NotACommit => f.write_str("not a commit"),
            ShortfallReason::NotReachable => f.write_str("not reachable"),
            ShortfallReason::NotNew => f.write_str("not new"),
            ShortfallReason::GitFailed => f.write_str("git failed"),
            ShortfallReason::CommandExited(code) => write!(f, "command exited {code}"),
            ShortfallReason::CommandTimedOut => f.write_str("command timed out"),
            ShortfallReason::CommandFailed => f.write_str("command failed"),
        }
    }
}

/// The first requirement of a gate that an event did not meet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortfall {
    /// Which requirement failed, e.g. ``field `commit` `` or ``command `cargo test` ``.
    pub requirement: String,
    /// Why it failed.
    pub reason: ShortfallReason,
    /// Human-readable detail; for commands, the stderr tail.
    pub detail: String,
}

impl fmt::Display for Shortfall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.requirement, self.reason)?;
        if !self.detail.is_empty() {
            write!(f, ": {}", self.detail)?;
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Command runner
// ─────────────────────────────────────────────────────────────────────────────

/// A gate command to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCommandRequest {
    /// Topic of the gated event (used to label the run).
    pub event_topic: String,
    /// Command argv.
    pub command: Vec<String>,
    /// Working directory.
    pub workspace_root: PathBuf,
    /// Extra environment: `RALPH_EVENT_TOPIC` (capped at 1 KiB) and
    /// `RALPH_EVENT_PAYLOAD` (capped at 64 KiB), both with NULs removed.
    pub env: HashMap<String, String>,
    /// Timeout in seconds.
    pub timeout_seconds: u64,
}

/// Result of a gate command that ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCommandOutcome {
    /// Exit code (None when killed by a signal or on timeout).
    pub exit_code: Option<i32>,
    /// Whether the timeout fired.
    pub timed_out: bool,
    /// Captured stderr.
    pub stderr: String,
}

/// Runs gate commands. Injected so tests need no real processes.
pub trait GateCommandRunner {
    /// Runs the command; `Err` means it could not be started.
    fn run(&self, request: GateCommandRequest) -> Result<GateCommandOutcome, String>;
}

/// Gate command runner backed by the lifecycle hook executor.
#[derive(Debug, Clone, Default)]
pub struct HookGateCommandRunner<E = HookExecutor> {
    executor: E,
}

impl HookGateCommandRunner<HookExecutor> {
    /// Creates a runner over the default hook executor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            executor: HookExecutor::new(),
        }
    }
}

impl<E: HookExecutorContract> HookGateCommandRunner<E> {
    /// Creates a runner over a specific hook executor.
    pub fn with_executor(executor: E) -> Self {
        Self { executor }
    }
}

impl<E: HookExecutorContract> GateCommandRunner for HookGateCommandRunner<E> {
    fn run(&self, request: GateCommandRequest) -> Result<GateCommandOutcome, String> {
        // The payload travels only in `RALPH_EVENT_PAYLOAD`. Repeating it on
        // stdin lets a large payload fill the pipe before the executor starts
        // its output collectors and timeout, which deadlocks a chatty command.
        let stdin_payload = serde_json::json!({ "topic": request.event_topic });
        let result = self
            .executor
            .run(HookRunRequest {
                phase_event: "gate".to_string(),
                hook_name: format!("gate:{}", request.event_topic),
                command: request.command,
                workspace_root: request.workspace_root,
                cwd: None,
                env: request.env,
                timeout_seconds: request.timeout_seconds,
                max_output_bytes: GATE_COMMAND_MAX_OUTPUT_BYTES,
                stdin_payload,
                // Gates run inside iterations, after Ralph's signal handlers
                // are installed, so a timeout can kill the command's tree.
                own_process_group: true,
            })
            .map_err(|e| e.to_string())?;
        Ok(GateCommandOutcome {
            exit_code: result.exit_code,
            timed_out: result.timed_out,
            stderr: result.stderr.content,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Evaluation
// ─────────────────────────────────────────────────────────────────────────────

/// HEAD as recorded at iteration start, for `new` commit checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartHead {
    /// HEAD's branch has no commit yet, so every commit counts as new.
    Unborn,
    /// HEAD resolved to this commit.
    Commit(String),
    /// HEAD could not be recorded (git failed, HEAD broken, or the loop never
    /// recorded it). `new` checks fail closed with this detail.
    Unknown(String),
}

/// Whether any gate has a `new` commit check, so the loop must record HEAD
/// at iteration start. Without one, no git process runs per iteration.
pub fn needs_start_head(gates: &[GateConfig]) -> bool {
    gates
        .iter()
        .flat_map(|gate| &gate.requires)
        .any(|requirement| {
            matches!(
                requirement,
                GateRequirement::Field(FieldRequirement { verify: Some(verify), .. })
                    if verify.commit().new
            )
        })
}

/// Records HEAD in `root` for later `new` commit checks.
///
/// `Unborn` only when HEAD is a symbolic ref to a branch that does not exist
/// yet; a branch that points at a missing or unreadable object is `Unknown`,
/// so a broken repo can never make every commit look new.
pub fn record_start_head(root: &Path) -> StartHead {
    match resolve_commit(root, "HEAD") {
        Ok(Some(id)) => StartHead::Commit(id),
        Err(error) => StartHead::Unknown(error),
        Ok(None) => match head_is_unborn(root) {
            Ok(true) => StartHead::Unborn,
            Ok(false) => StartHead::Unknown("HEAD does not resolve to a commit".to_string()),
            Err(error) => StartHead::Unknown(error),
        },
    }
}

/// Whether HEAD names a branch that has no ref yet (a fresh repo or orphan
/// branch). `Err` means git could not be run.
fn head_is_unborn(root: &Path) -> Result<bool, String> {
    let symbolic = run_git(root, &["symbolic-ref", "--quiet", "HEAD"])?;
    if !symbolic.status.success() {
        // A detached HEAD that does not resolve, or a branch whose ref file is
        // garbage (git 2.50 fails `symbolic-ref` there): broken, not unborn.
        return Ok(false);
    }
    let branch = String::from_utf8_lossy(&symbolic.stdout).trim().to_string();
    // Exit 1 with nothing on stderr: the ref does not exist. A ref holding a
    // missing object resolves (exit 0). Requiring empty stderr is a backstop
    // for a broken ref that git only warns about ("ignoring broken ref").
    let lookup = run_git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &branch,
        ],
    )?;
    Ok(lookup.status.code() == Some(1) && lookup.stderr.is_empty())
}

/// What gate evaluation needs from the running loop.
pub struct GateContext<'a> {
    /// Repository / workspace root: git checks and commands run here.
    pub workspace_root: &'a Path,
    /// HEAD recorded at iteration start, for `new` commit checks.
    pub iteration_start_head: &'a StartHead,
    /// Runs `command` requirements.
    pub runner: &'a dyn GateCommandRunner,
}

/// Checks an event against a gate. Returns the first unmet requirement.
pub fn evaluate(
    gate: &GateConfig,
    topic: &str,
    payload: &str,
    ctx: &GateContext<'_>,
) -> Result<(), Shortfall> {
    for requirement in &gate.requires {
        match requirement {
            GateRequirement::Field(field) => check_field(field, payload, ctx)?,
            GateRequirement::Command(command) => check_command(command, topic, payload, ctx)?,
        }
    }
    Ok(())
}

fn check_field(
    req: &FieldRequirement,
    payload: &str,
    ctx: &GateContext<'_>,
) -> Result<(), Shortfall> {
    let requirement = format!("field `{}`", req.field);
    let fail = |reason, detail: String| Shortfall {
        requirement: requirement.clone(),
        reason,
        detail,
    };

    let Some(entry) = parse_evidence_entry(payload, &req.field) else {
        return Err(fail(
            ShortfallReason::Missing,
            format!("`{}` not found in payload", req.field),
        ));
    };

    if let Some(expected) = &req.status {
        // A bare value that is itself the status word (`tests=passed`) counts.
        let actual = entry.status.as_deref().unwrap_or(entry.raw.trim());
        if actual != expected {
            return Err(fail(
                ShortfallReason::Status,
                format!("expected status `{expected}`, got `{}`", shown(actual)),
            ));
        }
    }

    if req.min.is_some() || req.max.is_some() {
        let Some(value) = entry.numeric else {
            return Err(fail(
                ShortfallReason::Threshold,
                format!("`{}` is not a number", shown(&entry.raw)),
            ));
        };
        if let Some(min) = req.min
            && value < min
        {
            return Err(fail(
                ShortfallReason::Threshold,
                format!("value {value} is below min {min}"),
            ));
        }
        if let Some(max) = req.max
            && value > max
        {
            return Err(fail(
                ShortfallReason::Threshold,
                format!("value {value} is above max {max}"),
            ));
        }
    }

    if let Some(verify) = &req.verify {
        check_commit(&verify.commit(), entry.raw.trim(), ctx).map_err(|(reason, detail)| {
            Shortfall {
                requirement: requirement.clone(),
                reason,
                detail,
            }
        })?;
    }

    Ok(())
}

fn check_commit(
    verify: &CommitVerify,
    value: &str,
    ctx: &GateContext<'_>,
) -> Result<(), (ShortfallReason, String)> {
    let root = ctx.workspace_root;
    let git_failed = |detail: String| (ShortfallReason::GitFailed, detail);

    let shown_value = shown(value);
    // Reject anything git could read as an option or a multi-word expression,
    // and anything that cannot go into argv at all (NUL, oversized): those are
    // the agent's evidence being wrong, not git failing.
    let plausible = !value.is_empty()
        && value.len() <= COMMIT_VALUE_MAX_BYTES
        && !value.starts_with('-')
        && !value.chars().any(|c| c.is_whitespace() || c == '\0');
    let not_a_commit = || {
        (
            ShortfallReason::NotACommit,
            format!("`{shown_value}` does not resolve to a commit"),
        )
    };
    if !plausible {
        return Err(not_a_commit());
    }

    // Resolve the config-owned ref first. Its exit codes decide whether git
    // works in this workspace; once it has, any rev-parse failure on the
    // agent's value is that value's fault, since rev-parse exit codes do not
    // reliably separate a bad rev (e.g. `HEAD@{100}` exits 128) from git
    // being broken.
    let reachable_from = &verify.reachable_from;
    let target = resolve_commit(root, reachable_from).map_err(git_failed)?;
    let commit = resolve_agent_commit(root, value)
        .map_err(git_failed)?
        .ok_or_else(not_a_commit)?;

    let not_reachable = |why: &str| {
        (
            ShortfallReason::NotReachable,
            format!("`{shown_value}` is not reachable from `{reachable_from}`{why}"),
        )
    };
    let target = target.ok_or_else(|| not_reachable(" (ref does not resolve to a commit)"))?;
    if !is_ancestor(root, &commit, &target).map_err(git_failed)? {
        return Err(not_reachable(""));
    }

    if verify.new {
        let start_head = match ctx.iteration_start_head {
            StartHead::Unborn => return Ok(()),
            StartHead::Commit(head) => head.as_str(),
            StartHead::Unknown(why) => {
                return Err(git_failed(format!(
                    "HEAD at iteration start is unknown, so `{shown_value}` cannot be checked as new: {why}"
                )));
            }
        };
        // Ralph recorded this HEAD itself, so failing to resolve it is an
        // error, not evidence that the commit is new.
        let start = resolve_commit(root, start_head)
            .map_err(git_failed)?
            .ok_or_else(|| {
                git_failed(format!(
                    "iteration start HEAD `{start_head}` does not resolve to a commit"
                ))
            })?;
        if is_ancestor(root, &commit, &start).map_err(git_failed)? {
            return Err((
                ShortfallReason::NotNew,
                format!(
                    "`{shown_value}` was already reachable from HEAD at iteration start ({start_head})"
                ),
            ));
        }
    }

    Ok(())
}

/// Outcome of `git rev-parse --verify --quiet <rev>^{commit}`.
enum RevParse {
    Commit(String),
    /// Exit 1: the name does not resolve (silently or with a type-mismatch note).
    Missing,
    /// Any other exit code (or a signal): git errored, or the rev is bad in a
    /// way rev-parse reports as fatal.
    Error(Output),
}

/// `Err` means git could not be run at all.
fn rev_parse_commit(root: &Path, rev: &str) -> Result<RevParse, String> {
    let rev = format!("{rev}^{{commit}}");
    let output = run_git(
        root,
        &["rev-parse", "--verify", "--quiet", "--end-of-options", &rev],
    )?;
    Ok(match output.status.code() {
        // Only a bare object id is a commit; `^HEAD` resolves too but prints
        // `^<sha>`, a negated revision.
        Some(0) => {
            let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let is_object_id =
                matches!(id.len(), 40 | 64) && id.bytes().all(|b| b.is_ascii_hexdigit());
            if is_object_id {
                RevParse::Commit(id)
            } else {
                RevParse::Missing
            }
        }
        Some(1) => RevParse::Missing,
        _ => RevParse::Error(output),
    })
}

/// Resolves a Ralph- or config-owned `rev` to a full commit hash. `Ok(None)`
/// means it does not name a commit; `Err` means git itself failed.
fn resolve_commit(root: &Path, rev: &str) -> Result<Option<String>, String> {
    match rev_parse_commit(root, rev)? {
        RevParse::Commit(id) => Ok(Some(id)),
        RevParse::Missing => Ok(None),
        RevParse::Error(quiet) => {
            // `--quiet` can swallow the fatal message (reflog overflow does),
            // so rerun without it to give the detail git's stderr. Dropping
            // `--quiet` up front would turn "not found" into exit 128 too.
            let rev = format!("{rev}^{{commit}}");
            let args = ["rev-parse", "--verify", "--end-of-options", &rev];
            let loud = run_git(root, &args).ok().filter(|o| !o.stderr.is_empty());
            Err(git_error(
                &["rev-parse", &rev],
                loud.as_ref().unwrap_or(&quiet),
            ))
        }
    }
}

/// Resolves the agent's `rev` once git is known to work: any rev-parse
/// failure means the value does not name a commit. `Err` only when git could
/// not be run.
fn resolve_agent_commit(root: &Path, rev: &str) -> Result<Option<String>, String> {
    Ok(match rev_parse_commit(root, rev)? {
        RevParse::Commit(id) => Some(id),
        RevParse::Missing | RevParse::Error(_) => None,
    })
}

/// `git merge-base --is-ancestor`: exit 0 = yes, 1 = no, anything else is an error.
fn is_ancestor(root: &Path, commit: &str, of: &str) -> Result<bool, String> {
    let args = ["merge-base", "--is-ancestor", commit, of];
    let output = run_git(root, &args)?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(git_error(&args, &output)),
    }
}

fn git_error(args: &[&str], output: &Output) -> String {
    let status = output
        .status
        .code()
        .map_or_else(|| "killed by signal".to_string(), |c| format!("exit {c}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "`git {}` failed ({status}): {}",
        args.join(" "),
        stderr_tail(stderr.trim())
    )
}

fn run_git(root: &Path, args: &[&str]) -> Result<Output, String> {
    run_git_program("git", root, args)
}

fn run_git_program(program: &str, root: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new(program)
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not run git: {e}"))
}

fn check_command(
    req: &CommandRequirement,
    topic: &str,
    payload: &str,
    ctx: &GateContext<'_>,
) -> Result<(), Shortfall> {
    let requirement = format!("command `{}`", req.command.join(" "));
    // Event text is agent-controlled; a NUL or an oversized value would make
    // spawn fail on every retry, an unpassable gate unrelated to evidence.
    let topic = sanitize_for_process(topic, GATE_EVENT_TOPIC_ENV_MAX_BYTES);
    let env = HashMap::from([
        ("RALPH_EVENT_TOPIC".to_string(), topic.clone()),
        (
            "RALPH_EVENT_PAYLOAD".to_string(),
            sanitize_for_process(payload, GATE_EVENT_PAYLOAD_ENV_MAX_BYTES),
        ),
    ]);
    let outcome = ctx.runner.run(GateCommandRequest {
        event_topic: topic,
        command: req.command.clone(),
        workspace_root: ctx.workspace_root.to_path_buf(),
        env,
        timeout_seconds: req.timeout.unwrap_or(DEFAULT_GATE_COMMAND_TIMEOUT_SECS),
    });

    let (reason, detail) = match outcome {
        Err(error) => (ShortfallReason::CommandFailed, error),
        Ok(outcome) if outcome.timed_out => (
            ShortfallReason::CommandTimedOut,
            stderr_tail(&outcome.stderr).to_string(),
        ),
        Ok(GateCommandOutcome {
            exit_code: Some(0), ..
        }) => return Ok(()),
        Ok(GateCommandOutcome {
            exit_code: Some(code),
            stderr,
            ..
        }) => (
            ShortfallReason::CommandExited(code),
            stderr_tail(&stderr).to_string(),
        ),
        Ok(GateCommandOutcome {
            exit_code: None,
            stderr,
            ..
        }) => (
            ShortfallReason::CommandFailed,
            match stderr_tail(&stderr) {
                "" => "killed by a signal".to_string(),
                tail => format!("killed by a signal: {tail}"),
            },
        ),
    };
    Err(Shortfall {
        requirement,
        reason,
        detail,
    })
}

/// The last [`GATE_STDERR_TAIL_BYTES`] of `stderr`, trimmed, on a char boundary.
fn stderr_tail(stderr: &str) -> &str {
    let stderr = stderr.trim_end();
    let mut start = stderr.len().saturating_sub(GATE_STDERR_TAIL_BYTES);
    while !stderr.is_char_boundary(start) {
        start += 1;
    }
    stderr[start..].trim_start()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FieldVerify, VerifyKeyword};
    use std::cell::RefCell;

    // ── Fixtures ─────────────────────────────────────────────────────────────

    /// Runner that records calls and returns a fixed outcome.
    struct FakeRunner {
        outcome: Result<GateCommandOutcome, String>,
        calls: RefCell<Vec<GateCommandRequest>>,
    }

    impl FakeRunner {
        fn exiting(code: i32, stderr: &str) -> Self {
            Self {
                outcome: Ok(GateCommandOutcome {
                    exit_code: Some(code),
                    timed_out: false,
                    stderr: stderr.to_string(),
                }),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl GateCommandRunner for FakeRunner {
        fn run(&self, request: GateCommandRequest) -> Result<GateCommandOutcome, String> {
            self.calls.borrow_mut().push(request);
            self.outcome.clone()
        }
    }

    fn field(name: &str) -> FieldRequirement {
        FieldRequirement {
            field: name.to_string(),
            status: None,
            min: None,
            max: None,
            verify: None,
        }
    }

    fn gate(requires: Vec<GateRequirement>) -> GateConfig {
        GateConfig {
            event: "review.ready".to_string(),
            requires,
            blocked: None,
        }
    }

    fn command(argv: &[&str]) -> GateRequirement {
        GateRequirement::Command(CommandRequirement {
            command: argv.iter().map(|s| s.to_string()).collect(),
            timeout: None,
        })
    }

    fn eval_with(
        requires: Vec<GateRequirement>,
        payload: &str,
        runner: &dyn GateCommandRunner,
    ) -> Result<(), Shortfall> {
        let root = std::env::temp_dir();
        let ctx = GateContext {
            workspace_root: &root,
            iteration_start_head: &StartHead::Unborn,
            runner,
        };
        evaluate(&gate(requires), "review.ready", payload, &ctx)
    }

    fn eval(requires: Vec<GateRequirement>, payload: &str) -> Result<(), Shortfall> {
        eval_with(requires, payload, &FakeRunner::exiting(0, ""))
    }

    fn reason(result: Result<(), Shortfall>) -> ShortfallReason {
        result.expect_err("expected a shortfall").reason
    }

    // ── Payload parsing ──────────────────────────────────────────────────────

    #[test]
    fn parses_json_scalar_values() {
        let payload = r#"{"commit": "abc123", "count": 0, "ok": true, "pct": "87%"}"#;
        let commit = parse_evidence_entry(payload, "commit").unwrap();
        assert_eq!(commit.raw, "abc123");
        assert_eq!(commit.numeric, None);
        assert_eq!(
            parse_evidence_entry(payload, "count").unwrap().numeric,
            Some(0.0)
        );
        assert_eq!(parse_evidence_entry(payload, "ok").unwrap().raw, "true");
        assert_eq!(
            parse_evidence_entry(payload, "pct").unwrap().numeric,
            Some(87.0)
        );
        assert!(parse_evidence_entry(payload, "absent").is_none());
    }

    #[test]
    fn parses_json_typed_form() {
        let payload = r#"{"tests": {"value": 42, "status": "passed"}}"#;
        let entry = parse_evidence_entry(payload, "tests").unwrap();
        assert_eq!(entry.raw, "42");
        assert_eq!(entry.numeric, Some(42.0));
        assert_eq!(entry.status.as_deref(), Some("passed"));
    }

    #[test]
    fn json_vacuous_values_are_missing() {
        let payload = r#"{"a": "", "b": "  ", "c": false, "d": null, "e": [1], "f": {}, "g": {"value": " "}}"#;
        for key in ["a", "b", "c", "d", "e", "f", "g"] {
            assert!(
                parse_evidence_entry(payload, key).is_none(),
                "{key} should be vacuous"
            );
        }
    }

    #[test]
    fn parses_tokens() {
        let payload = r#"commit=abc123, tests=5 passed; coverage="87%" note='hi there'"#;
        assert_eq!(
            parse_evidence_entry(payload, "commit").unwrap().raw,
            "abc123"
        );

        let tests = parse_evidence_entry(payload, "tests").unwrap();
        assert_eq!(tests.raw, "5 passed");
        assert_eq!(tests.numeric, Some(5.0));
        assert_eq!(tests.status.as_deref(), Some("passed"));

        assert_eq!(
            parse_evidence_entry(payload, "coverage").unwrap().numeric,
            Some(87.0)
        );
        assert_eq!(
            parse_evidence_entry(payload, "note").unwrap().raw,
            "hi there"
        );
        assert!(parse_evidence_entry(payload, "absent").is_none());
    }

    #[test]
    fn token_vacuous_values_are_missing() {
        assert!(parse_evidence_entry(r#"commit="" other=1"#, "commit").is_none());
        assert!(parse_evidence_entry("", "commit").is_none());
        assert!(parse_evidence_entry("just prose, no tokens", "commit").is_none());
    }

    #[test]
    fn brace_payload_that_is_not_json_falls_back_to_tokens() {
        let entry = parse_evidence_entry("{not json} commit=abc", "commit").unwrap();
        assert_eq!(entry.raw, "abc");
    }

    // ── Field requirements ───────────────────────────────────────────────────

    #[test]
    fn field_presence() {
        let req = || vec![GateRequirement::Field(field("commit"))];
        assert!(eval(req(), "commit=abc").is_ok());
        let shortfall = eval(req(), "nothing here").unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::Missing);
        assert_eq!(shortfall.requirement, "field `commit`");
        assert_eq!(shortfall.reason.to_string(), "missing");
    }

    #[test]
    fn field_status() {
        let req = || {
            vec![GateRequirement::Field(FieldRequirement {
                status: Some("passed".to_string()),
                ..field("tests")
            })]
        };
        assert!(eval(req(), "tests=12 passed").is_ok());
        assert!(eval(req(), "tests=passed").is_ok());
        assert!(eval(req(), r#"{"tests": {"value": "12", "status": "passed"}}"#).is_ok());
        assert!(eval(req(), r#"{"tests": "passed"}"#).is_ok());

        let shortfall = eval(req(), "tests=3 failed").unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::Status);
        assert!(shortfall.detail.contains("failed"), "{shortfall}");
        // A value with no status word does not carry the required status.
        assert_eq!(reason(eval(req(), "tests=12")), ShortfallReason::Status);
        assert_eq!(reason(eval(req(), "")), ShortfallReason::Missing);
    }

    #[test]
    fn field_threshold() {
        let req = || {
            vec![GateRequirement::Field(FieldRequirement {
                min: Some(80.0),
                max: Some(100.0),
                ..field("coverage")
            })]
        };
        assert!(eval(req(), "coverage=80").is_ok());
        assert!(eval(req(), "coverage=92.5%").is_ok());
        assert!(eval(req(), r#"{"coverage": 100}"#).is_ok());

        let below = eval(req(), "coverage=79.9%").unwrap_err();
        assert_eq!(below.reason, ShortfallReason::Threshold);
        assert!(below.detail.contains("below min 80"), "{below}");
        let above = eval(req(), "coverage=101").unwrap_err();
        assert!(above.detail.contains("above max 100"), "{above}");
        assert_eq!(
            reason(eval(req(), "coverage=high")),
            ShortfallReason::Threshold
        );
    }

    // ── Commit requirements (real git) ───────────────────────────────────────

    struct Repo {
        dir: tempfile::TempDir,
    }

    impl Repo {
        fn new() -> Self {
            let repo = Self {
                dir: tempfile::tempdir().unwrap(),
            };
            repo.git(&["init", "-q", "-b", "main"]);
            repo
        }

        fn git(&self, args: &[&str]) -> String {
            let output = Command::new("git")
                .arg("-C")
                .arg(self.dir.path())
                .args([
                    "-c",
                    "user.name=Gate Test",
                    "-c",
                    "user.email=gate@test",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap().trim().to_string()
        }

        fn commit(&self, message: &str) -> String {
            self.git(&["commit", "-q", "--allow-empty", "-m", message]);
            self.git(&["rev-parse", "HEAD"])
        }

        fn check(
            &self,
            verify: FieldVerify,
            value: &str,
            start_head: Option<&str>,
        ) -> Result<(), Shortfall> {
            self.check_payload(verify, &format!("commit={value}"), start_head)
        }

        fn check_payload(
            &self,
            verify: FieldVerify,
            payload: &str,
            start_head: Option<&str>,
        ) -> Result<(), Shortfall> {
            let start_head = start_head.map_or(StartHead::Unborn, |h| StartHead::Commit(h.into()));
            self.check_against(verify, payload, &start_head)
        }

        fn check_against(
            &self,
            verify: FieldVerify,
            payload: &str,
            start_head: &StartHead,
        ) -> Result<(), Shortfall> {
            let runner = FakeRunner::exiting(0, "");
            let ctx = GateContext {
                workspace_root: self.dir.path(),
                iteration_start_head: start_head,
                runner: &runner,
            };
            let gate = GateConfig {
                event: "foundation.done".to_string(),
                requires: vec![GateRequirement::Field(FieldRequirement {
                    verify: Some(verify),
                    ..field("commit")
                })],
                blocked: None,
            };
            evaluate(&gate, "foundation.done", payload, &ctx)
        }
    }

    fn verify_commit() -> FieldVerify {
        FieldVerify::Keyword(VerifyKeyword::Commit)
    }

    fn verify_with(reachable_from: &str, new: bool) -> FieldVerify {
        FieldVerify::Commit {
            commit: CommitVerify {
                reachable_from: reachable_from.to_string(),
                new,
            },
        }
    }

    #[test]
    fn commit_created_this_iteration_passes() {
        let repo = Repo::new();
        let start = repo.commit("start");
        let new = repo.commit("work");
        assert!(repo.check(verify_commit(), &new, Some(&start)).is_ok());
        assert!(
            repo.check(verify_commit(), &new[..10], Some(&start))
                .is_ok()
        );
        // No HEAD at iteration start: every commit is new.
        assert!(repo.check(verify_commit(), &start, None).is_ok());
    }

    #[test]
    fn non_commit_values_are_not_a_commit() {
        let repo = Repo::new();
        let start = repo.commit("start");
        let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
        let caret_start = format!("^{start}");
        for value in [
            "deadbeefdeadbeef",
            "--help",
            "not-a-ref",
            tree.as_str(),
            // `rev-parse --verify` accepts these and prints `^<sha>`.
            "^HEAD",
            caret_start.as_str(),
        ] {
            let shortfall = repo
                .check(verify_commit(), value, Some(&start))
                .unwrap_err();
            assert_eq!(shortfall.reason, ShortfallReason::NotACommit, "{value}");
            assert_eq!(shortfall.reason.to_string(), "not a commit");
        }
    }

    #[test]
    fn agent_values_git_rejects_with_an_error_exit_are_not_a_commit() {
        // `rev-parse --verify --quiet` exits 128 (not 1) for a reflog entry past
        // the end of the log. Git works here, so it is the agent's evidence.
        let repo = Repo::new();
        let start = repo.commit("start");
        for value in [
            "HEAD@{100}",
            "main@{5}",
            "@{100}",
            "HEAD@{999999999999999999999}",
        ] {
            let shortfall = repo
                .check(verify_commit(), value, Some(&start))
                .unwrap_err();
            assert_eq!(shortfall.reason, ShortfallReason::NotACommit, "{shortfall}");
        }
    }

    #[test]
    fn config_ref_git_errors_on_is_a_git_failure_with_stderr() {
        // Same exit-128 rev, but owned by config: git's message must survive
        // `--quiet` so the detail says what went wrong.
        let repo = Repo::new();
        let start = repo.commit("start");
        let shortfall = repo
            .check(verify_with("main@{5}", true), &start, None)
            .unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::GitFailed);
        assert!(
            shortfall.detail.contains("only has 1 entries"),
            "{shortfall}"
        );
    }

    #[test]
    fn commit_already_reachable_at_iteration_start_is_not_new() {
        let repo = Repo::new();
        let old = repo.commit("old");
        let start = repo.commit("start");
        for value in [&old, &start] {
            let shortfall = repo
                .check(verify_commit(), value, Some(&start))
                .unwrap_err();
            assert_eq!(shortfall.reason, ShortfallReason::NotNew);
            assert_eq!(shortfall.reason.to_string(), "not new");
        }
        // `new: false` drops the condition.
        assert!(
            repo.check(verify_with("HEAD", false), &old, Some(&start))
                .is_ok()
        );
    }

    #[test]
    fn commit_not_reachable_from_ref() {
        let repo = Repo::new();
        let start = repo.commit("start");
        repo.git(&["checkout", "-q", "-b", "side"]);
        let side = repo.commit("side work");
        repo.git(&["checkout", "-q", "main"]);

        let shortfall = repo
            .check(verify_commit(), &side, Some(&start))
            .unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::NotReachable);
        assert!(shortfall.detail.contains("`HEAD`"), "{shortfall}");

        // `reachable_from` picks the ref.
        assert!(
            repo.check(verify_with("side", true), &side, Some(&start))
                .is_ok()
        );
        // An unknown ref is never reachable.
        assert_eq!(
            reason(repo.check(verify_with("no-such-ref", true), &side, Some(&start))),
            ShortfallReason::NotReachable
        );
    }

    #[test]
    fn record_start_head_resolves_commits_and_unborn_branches() {
        let repo = Repo::new();
        assert_eq!(record_start_head(repo.dir.path()), StartHead::Unborn);
        let head = repo.commit("first");
        assert_eq!(record_start_head(repo.dir.path()), StartHead::Commit(head));
        // An orphan branch has no commit yet either.
        repo.git(&["checkout", "-q", "--orphan", "fresh"]);
        assert_eq!(record_start_head(repo.dir.path()), StartHead::Unborn);
    }

    #[test]
    fn record_start_head_is_unknown_when_head_is_broken() {
        // A branch pointing at a missing object, a garbage ref file, and a
        // detached HEAD at a missing object are broken, not unborn: treating
        // them as unborn would make every commit look new.
        let missing = "0123456789012345678901234567890123456789";
        let repo = Repo::new();
        repo.commit("first");
        let branch_ref = repo.dir.path().join(".git/refs/heads/main");
        for (contents, label) in [
            (format!("{missing}\n"), "missing object"),
            ("garbage\n".to_string(), "garbage ref"),
        ] {
            std::fs::write(&branch_ref, contents).unwrap();
            let head = record_start_head(repo.dir.path());
            assert!(matches!(head, StartHead::Unknown(_)), "{label}: {head:?}");
        }
        std::fs::write(repo.dir.path().join(".git/HEAD"), format!("{missing}\n")).unwrap();
        let head = record_start_head(repo.dir.path());
        assert!(matches!(head, StartHead::Unknown(_)), "detached: {head:?}");

        let outside = tempfile::tempdir().unwrap();
        match record_start_head(outside.path()) {
            StartHead::Unknown(why) => assert!(why.contains("not a git repository"), "{why}"),
            other => panic!("expected Unknown outside a repo, got {other:?}"),
        }
        match record_start_head(Path::new("/definitely/not/a/dir")) {
            StartHead::Unknown(why) => assert!(!why.is_empty()),
            other => panic!("expected Unknown for a missing dir, got {other:?}"),
        }
    }

    #[test]
    fn unknown_start_head_fails_new_checks_closed() {
        let repo = Repo::new();
        repo.commit("start");
        let work = repo.commit("work");
        let unknown = StartHead::Unknown("HEAD was not recorded".to_string());
        let shortfall = repo
            .check_against(verify_commit(), &format!("commit={work}"), &unknown)
            .unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::GitFailed);
        assert!(
            shortfall.detail.contains("HEAD was not recorded"),
            "{shortfall}"
        );
        // `new: false` never needs the start HEAD.
        assert!(
            repo.check_against(
                verify_with("HEAD", false),
                &format!("commit={work}"),
                &unknown
            )
            .is_ok()
        );
        // Every commit is new on an unborn start.
        assert!(
            repo.check_against(
                verify_commit(),
                &format!("commit={work}"),
                &StartHead::Unborn
            )
            .is_ok()
        );
    }

    #[test]
    fn invalid_iteration_start_head_fails_closed() {
        let repo = Repo::new();
        let old = repo.commit("old");
        repo.commit("start");
        let shortfall = repo
            .check(verify_commit(), &old, Some("not-a-real-ref"))
            .unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::GitFailed);
        assert_eq!(shortfall.reason.to_string(), "git failed");
        assert!(shortfall.detail.contains("not-a-real-ref"), "{shortfall}");
    }

    #[test]
    fn git_error_on_reachable_check_fails_closed() {
        // Delete a shared ancestor so `merge-base` errors (exit 128) while
        // both commits still resolve.
        let repo = Repo::new();
        let base = repo.commit("base");
        let start = repo.commit("start");
        repo.git(&["checkout", "-q", "-b", "side", &base]);
        let side = repo.commit("side work");
        repo.git(&["checkout", "-q", "main"]);
        std::fs::remove_file(
            repo.dir
                .path()
                .join(".git/objects")
                .join(&base[..2])
                .join(&base[2..]),
        )
        .unwrap();

        let shortfall = repo
            .check(verify_commit(), &side, Some(&start))
            .unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::GitFailed);
        assert!(
            shortfall.detail.contains("could not parse commit"),
            "{shortfall}"
        );
    }

    #[test]
    fn workspace_outside_a_repo_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let runner = FakeRunner::exiting(0, "");
        let ctx = GateContext {
            workspace_root: dir.path(),
            iteration_start_head: &StartHead::Unborn,
            runner: &runner,
        };
        let gate = gate(vec![GateRequirement::Field(FieldRequirement {
            verify: Some(verify_commit()),
            ..field("commit")
        })]);
        let shortfall = evaluate(&gate, "review.ready", "commit=abc1234", &ctx).unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::GitFailed);
        assert!(
            shortfall.detail.contains("not a git repository"),
            "{shortfall}"
        );
    }

    #[test]
    fn git_that_cannot_start_is_a_git_failure() {
        let dir = tempfile::tempdir().unwrap();
        let err = run_git_program("ralph-no-such-git-binary", dir.path(), &["status"]).unwrap_err();
        assert!(err.contains("could not run git"), "{err}");
    }

    // ── Command requirements (fake runner) ───────────────────────────────────

    #[test]
    fn command_success_passes_and_receives_event_env() {
        let runner = FakeRunner::exiting(0, "");
        assert!(eval_with(vec![command(&["make", "check"])], "tests=passed", &runner).is_ok());

        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.command, vec!["make", "check"]);
        assert_eq!(call.env["RALPH_EVENT_TOPIC"], "review.ready");
        assert_eq!(call.env["RALPH_EVENT_PAYLOAD"], "tests=passed");
        assert_eq!(call.timeout_seconds, DEFAULT_GATE_COMMAND_TIMEOUT_SECS);
    }

    #[test]
    fn command_nonzero_exit_reports_code_and_stderr_tail() {
        let long_stderr = format!("{}\nFAILED: test_x\n", "noise ".repeat(2000));
        let runner = FakeRunner::exiting(1, &long_stderr);
        let shortfall = eval_with(vec![command(&["make", "check"])], "", &runner).unwrap_err();

        assert_eq!(shortfall.reason, ShortfallReason::CommandExited(1));
        assert_eq!(shortfall.reason.to_string(), "command exited 1");
        assert_eq!(shortfall.requirement, "command `make check`");
        assert!(
            shortfall.detail.ends_with("FAILED: test_x"),
            "{}",
            shortfall.detail
        );
        assert!(shortfall.detail.len() <= GATE_STDERR_TAIL_BYTES);
    }

    #[test]
    fn command_timeout_and_spawn_failure() {
        let timed_out = FakeRunner {
            outcome: Ok(GateCommandOutcome {
                exit_code: None,
                timed_out: true,
                stderr: String::new(),
            }),
            calls: RefCell::new(Vec::new()),
        };
        assert_eq!(
            reason(eval_with(vec![command(&["slow"])], "", &timed_out)),
            ShortfallReason::CommandTimedOut
        );

        let unspawnable = FakeRunner {
            outcome: Err("no such file".to_string()),
            calls: RefCell::new(Vec::new()),
        };
        let shortfall = eval_with(vec![command(&["missing"])], "", &unspawnable).unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::CommandFailed);
        assert!(shortfall.detail.contains("no such file"));
    }

    #[test]
    fn command_never_runs_after_an_earlier_field_failure() {
        let runner = FakeRunner::exiting(0, "");
        let requires = vec![
            GateRequirement::Field(field("tests")),
            command(&["expensive", "suite"]),
        ];
        let shortfall = eval_with(requires, "lint=ok", &runner).unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::Missing);
        assert!(runner.calls.borrow().is_empty(), "command must not run");
    }

    #[test]
    fn all_requirements_must_pass_in_order() {
        let runner = FakeRunner::exiting(2, "boom");
        let requires = vec![
            GateRequirement::Field(field("tests")),
            command(&["first"]),
            command(&["second"]),
        ];
        let shortfall = eval_with(requires, "tests=ok", &runner).unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::CommandExited(2));
        assert_eq!(
            shortfall.to_string(),
            "command `first`: command exited 2: boom"
        );
        assert_eq!(runner.calls.borrow().len(), 1, "stops at first failure");
    }

    #[test]
    fn empty_gate_passes() {
        assert!(eval(vec![], "").is_ok());
    }

    // ── HookExecutor-backed runner (real process) ────────────────────────────

    #[test]
    fn hook_runner_passes_env_and_captures_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let runner = HookGateCommandRunner::new();
        let ctx = GateContext {
            workspace_root: dir.path(),
            iteration_start_head: &StartHead::Unborn,
            runner: &runner,
        };

        let check_env = gate(vec![command(&[
            "sh",
            "-c",
            r#"test "$RALPH_EVENT_TOPIC" = review.ready && test "$RALPH_EVENT_PAYLOAD" = "tests=passed""#,
        ])]);
        assert!(evaluate(&check_env, "review.ready", "tests=passed", &ctx).is_ok());

        let failing = gate(vec![command(&[
            "sh",
            "-c",
            "echo 'unit tests failed' >&2; exit 1",
        ])]);
        let shortfall = evaluate(&failing, "review.ready", "", &ctx).unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::CommandExited(1));
        assert_eq!(shortfall.detail, "unit tests failed");
    }

    #[test]
    fn hook_runner_large_payload_and_chatty_command_do_not_deadlock() {
        // A payload larger than the pipe buffer plus a command that floods
        // stderr without reading stdin used to wedge the loop past its timeout.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let dir = tempfile::tempdir().unwrap();
            let runner = HookGateCommandRunner::new();
            let ctx = GateContext {
                workspace_root: dir.path(),
                iteration_start_head: &StartHead::Unborn,
                runner: &runner,
            };
            let chatty = gate(vec![GateRequirement::Command(CommandRequirement {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    r"head -c 200000 /dev/zero | tr '\0' x >&2; test ${#RALPH_EVENT_PAYLOAD} -eq 65536 || exit 9; exit 1".to_string(),
                ],
                timeout: Some(3),
            })]);
            let payload = "x".repeat(100 * 1024);
            let _ = tx.send(evaluate(&chatty, "review.ready", &payload, &ctx));
        });

        let result = rx
            .recv_timeout(std::time::Duration::from_secs(20))
            .expect("gate command must finish within its timeout");
        assert_eq!(reason(result), ShortfallReason::CommandExited(1));
    }

    #[test]
    fn payload_env_is_capped_on_a_char_boundary() {
        // 3-byte chars so the cap falls inside one.
        let payload = "€".repeat(100 * 1024);
        let runner = FakeRunner::exiting(0, "");
        assert!(eval_with(vec![command(&["true"])], &payload, &runner).is_ok());

        let calls = runner.calls.borrow();
        let env_payload = &calls[0].env["RALPH_EVENT_PAYLOAD"];
        assert!(env_payload.len() <= GATE_EVENT_PAYLOAD_ENV_MAX_BYTES);
        assert!(env_payload.len() > GATE_EVENT_PAYLOAD_ENV_MAX_BYTES - 3);
        assert!(payload.starts_with(env_payload.as_str()));

        // Short payloads pass through unchanged.
        drop(calls);
        let runner = FakeRunner::exiting(0, "");
        assert!(eval_with(vec![command(&["true"])], "tests=passed", &runner).is_ok());
        assert_eq!(
            runner.calls.borrow()[0].env["RALPH_EVENT_PAYLOAD"],
            "tests=passed"
        );
    }

    #[test]
    fn command_gate_passes_with_payload_over_env_string_limit() {
        // Linux rejects a single env string over 128 KiB (E2BIG) at spawn.
        let dir = tempfile::tempdir().unwrap();
        let runner = HookGateCommandRunner::new();
        let ctx = GateContext {
            workspace_root: dir.path(),
            iteration_start_head: &StartHead::Unborn,
            runner: &runner,
        };
        let check = gate(vec![GateRequirement::Command(CommandRequirement {
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                r#"test "${#RALPH_EVENT_PAYLOAD}" -le 65536"#.to_string(),
            ],
            timeout: Some(10),
        })]);
        let payload = "x".repeat(300 * 1024);
        assert_eq!(evaluate(&check, "review.ready", &payload, &ctx), Ok(()));

        let pass = gate(vec![command(&["true"])]);
        assert_eq!(evaluate(&pass, "review.ready", &payload, &ctx), Ok(()));
    }

    // ── Inputs that cannot go into argv / env ────────────────────────────────

    #[test]
    fn commit_values_that_cannot_go_into_argv_are_not_a_commit() {
        let repo = Repo::new();
        let start = repo.commit("start");
        let oversized = "a".repeat(200 * 1024);
        for payload in [
            format!("commit={oversized}"),
            // The JSON form decodes `\u0000` into a real NUL.
            r#"{"commit":"HEAD\u0000x"}"#.to_string(),
            format!(r#"{{"commit":"{start}\u0000"}}"#),
        ] {
            let shortfall = repo
                .check_payload(verify_commit(), &payload, Some(&start))
                .unwrap_err();
            assert_eq!(
                shortfall.reason,
                ShortfallReason::NotACommit,
                "{}",
                &payload[..payload.len().min(60)]
            );
            assert!(
                shortfall.detail.len() <= 2 * DETAIL_VALUE_MAX_CHARS,
                "detail is {} bytes",
                shortfall.detail.len()
            );
        }
    }

    #[test]
    fn echoed_values_in_details_are_bounded() {
        let repo = Repo::new();
        let start = repo.commit("start");
        let long_ref = format!("refs/heads/{}", "b".repeat(900));
        // Valid-looking but unresolvable, under the plausibility bound.
        let shortfall = repo
            .check_payload(verify_commit(), &format!("commit={long_ref}"), Some(&start))
            .unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::NotACommit);
        assert!(shortfall.detail.len() <= 2 * DETAIL_VALUE_MAX_CHARS);

        let huge = "z".repeat(100 * 1024);
        let status = FieldRequirement {
            status: Some("passed".to_string()),
            ..field("tests")
        };
        let threshold = FieldRequirement {
            min: Some(1.0),
            ..field("tests")
        };
        for req in [status, threshold] {
            let shortfall =
                eval(vec![GateRequirement::Field(req)], &format!("tests={huge}")).unwrap_err();
            assert!(
                shortfall.detail.len() <= 2 * DETAIL_VALUE_MAX_CHARS,
                "{} detail is {} bytes",
                shortfall.reason,
                shortfall.detail.len()
            );
        }
    }

    #[test]
    fn command_gate_passes_with_nul_in_payload_or_topic() {
        let dir = tempfile::tempdir().unwrap();
        let runner = HookGateCommandRunner::new();
        let ctx = GateContext {
            workspace_root: dir.path(),
            iteration_start_head: &StartHead::Unborn,
            runner: &runner,
        };
        let pass = gate(vec![command(&["true"])]);
        assert_eq!(evaluate(&pass, "review.ready", "a\u{0}b", &ctx), Ok(()));
        assert_eq!(evaluate(&pass, "review\u{0}.ready", "ok", &ctx), Ok(()));

        let sees = gate(vec![command(&[
            "sh",
            "-c",
            r#"test "$RALPH_EVENT_PAYLOAD" = ab && test "$RALPH_EVENT_TOPIC" = review.ready"#,
        ])]);
        assert_eq!(evaluate(&sees, "review.ready", "a\u{0}b", &ctx), Ok(()));
    }

    #[test]
    fn command_gate_passes_with_oversized_topic() {
        let dir = tempfile::tempdir().unwrap();
        let runner = HookGateCommandRunner::new();
        let ctx = GateContext {
            workspace_root: dir.path(),
            iteration_start_head: &StartHead::Unborn,
            runner: &runner,
        };
        let topic = "t".repeat(300 * 1024);
        let pass = gate(vec![command(&["true"])]);
        assert_eq!(evaluate(&pass, &topic, "ok", &ctx), Ok(()));
    }

    #[test]
    fn command_gate_timeout_kills_the_commands_children() {
        // `sleep` inherits the output pipes; without a process-group kill the
        // gate would wait the full 30s for it after the shell is killed.
        let dir = tempfile::tempdir().unwrap();
        let runner = HookGateCommandRunner::new();
        let ctx = GateContext {
            workspace_root: dir.path(),
            iteration_start_head: &StartHead::Unborn,
            runner: &runner,
        };
        let slow = gate(vec![GateRequirement::Command(CommandRequirement {
            command: ["sh", "-c", "sleep 30; true"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            timeout: Some(1),
        })]);
        let started = std::time::Instant::now();
        let shortfall = evaluate(&slow, "review.ready", "", &ctx).unwrap_err();
        assert_eq!(shortfall.reason, ShortfallReason::CommandTimedOut);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "gate took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn env_values_have_nuls_removed_before_the_cap() {
        let runner = FakeRunner::exiting(0, "");
        let payload = format!("{}{}", "\u{0}".repeat(10), "x".repeat(100 * 1024));
        assert!(eval_with(vec![command(&["true"])], &payload, &runner).is_ok());
        let calls = runner.calls.borrow();
        let env_payload = &calls[0].env["RALPH_EVENT_PAYLOAD"];
        assert!(!env_payload.contains('\0'));
        assert_eq!(env_payload.len(), GATE_EVENT_PAYLOAD_ENV_MAX_BYTES);
    }
}
