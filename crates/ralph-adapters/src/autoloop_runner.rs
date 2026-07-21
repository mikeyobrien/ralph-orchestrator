//! Adapter that drives the `autoloop` CLI as a subprocess.
//!
//! [`AutoloopRunner`] assembles an `autoloop run <preset> [options] "<prompt>"`
//! invocation, spawns it in a working directory, waits for completion, and parses the
//! "autoloops summary" block that autoloop prints to stdout on exit.
//!
//! # Invocation contract
//!
//! ```text
//! autoloop run <preset-dir> -b <backend> --set key=value ... "<prompt>"
//! ```
//!
//! Note the `-b` single-token gotcha: the backend value MUST be passed as ONE
//! argv element after `-b`. A multi-word string like `"node x.js"` shoved into a
//! single argv slot is mis-stored by autoloop and exits 127, so callers should
//! point `-b` at a single-token wrapper script.
//!
//! # Summary block
//!
//! ```text
//! autoloops summary
//! ===================
//! run_id: <id>
//! iterations: <n>
//! stop_reason: <reason>
//! journal: <abs path to .autoloop/journal.jsonl>
//! memory: <abs path to .autoloop/memory.jsonl>
//! ```

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use thiserror::Error;

/// Maximum number of trailing stderr bytes captured in error messages.
const STDERR_TAIL_BYTES: usize = 4096;

/// A parsed `autoloops summary` block.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoloopRunSummary {
    /// The autoloop run identifier.
    pub run_id: String,
    /// Number of iterations executed.
    pub iterations: u32,
    /// Why the loop stopped (e.g. `completed`, `max_iterations`, `stalled`).
    pub stop_reason: String,
    /// Cumulative run cost in USD (summed from journaled `backend.usage`); `0.0`
    /// when the backend reports no usage (e.g. `command`/`acp`) or the running
    /// autoloop predates the summary `cost_usd` field.
    pub cost_usd: f64,
    /// Absolute path to the run journal (`.autoloop/journal.jsonl`).
    pub journal: PathBuf,
    /// Absolute path to the run memory (`.autoloop/memory.jsonl`).
    pub memory: PathBuf,
}

/// Errors that can occur while running autoloop.
#[derive(Debug, Error)]
pub enum AutoloopRunError {
    /// The subprocess could not be spawned (e.g. binary not found).
    #[error("failed to spawn autoloop ({command}): {source}")]
    Spawn {
        /// The command that failed to spawn.
        command: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// The process exited non-zero. Captures the exit code and a tail of stderr.
    #[error("autoloop exited with code {code:?}; stderr tail:\n{stderr_tail}")]
    NonZeroExit {
        /// The process exit code, if available.
        code: Option<i32>,
        /// The last bytes of stderr, for diagnostics.
        stderr_tail: String,
    },

    /// The process succeeded but stdout did not contain a parseable summary block.
    #[error("could not parse 'autoloops summary' block from autoloop stdout")]
    UnparseableSummary,
}

/// How to invoke the `autoloop` binary.
#[derive(Debug, Clone, Default)]
pub enum AutoloopBin {
    /// Resolve `autoloop` from `PATH` (the default).
    #[default]
    PathLookup,
    /// Run `node <bin/autoloop>` explicitly (e.g. a checkout's `bin/autoloop`).
    Node(PathBuf),
    /// Run an explicit executable directly.
    Explicit(PathBuf),
}

/// Builder + runner for the `autoloop run` subprocess.
#[derive(Debug, Clone)]
pub struct AutoloopRunner {
    bin: AutoloopBin,
    preset_dir: PathBuf,
    prompt: String,
    working_dir: PathBuf,
    backend: Option<String>,
    /// Ordered `--set key=value` overrides.
    set_overrides: Vec<String>,
    /// Optional `--events <path>` NDJSON LoopEvent stream sink.
    events_path: Option<PathBuf>,
    /// Extra environment variables for the child process (inherited by its
    /// descendants, e.g. a backend wrapper script).
    env: Vec<(String, String)>,
    /// When set, spawn the child as the leader of a new process group so the
    /// whole subprocess tree (autoloop + its backend agent) can be signalled as
    /// a unit. Used by the TUI path to avoid orphaning the backend on Ctrl+C;
    /// left off for headless so the child stays in ralph's foreground group and
    /// receives terminal SIGINT normally.
    own_process_group: bool,
}

impl AutoloopRunner {
    /// Creates a runner for the given preset, prompt, and working directory.
    ///
    /// Defaults to resolving `autoloop` from `PATH`. Use [`Self::bin`] to
    /// override.
    pub fn new(
        preset_dir: impl Into<PathBuf>,
        prompt: impl Into<String>,
        working_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            bin: AutoloopBin::default(),
            preset_dir: preset_dir.into(),
            prompt: prompt.into(),
            working_dir: working_dir.into(),
            backend: None,
            set_overrides: Vec::new(),
            events_path: None,
            env: Vec::new(),
            own_process_group: false,
        }
    }

    /// Spawn the child as the leader of a new process group (Unix only), so its
    /// whole subprocess tree can be killed together via `killpg`. Off by default.
    pub fn own_process_group(mut self, yes: bool) -> Self {
        self.own_process_group = yes;
        self
    }

    /// Request the structured `--events <path>` NDJSON LoopEvent stream — the
    /// preferred observability channel (carries the resolved `progress` event
    /// and the machine-readable run result). Parse it with
    /// [`crate::autoloop_events`].
    pub fn events_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.events_path = Some(path.into());
        self
    }

    /// Overrides how the `autoloop` binary is invoked.
    pub fn bin(mut self, bin: AutoloopBin) -> Self {
        self.bin = bin;
        self
    }

    /// Sets the `-b <backend>` override. Passed as a single argv element.
    pub fn backend(mut self, backend: impl Into<String>) -> Self {
        self.backend = Some(backend.into());
        self
    }

    /// Appends a `--set key=value` override. Order is preserved.
    pub fn set_override(mut self, key: &str, value: &str) -> Self {
        self.set_overrides.push(format!("{key}={value}"));
        self
    }

    /// Convenience for `--set event_loop.max_iterations=<n>`.
    pub fn max_iterations(self, n: u32) -> Self {
        self.set_override("event_loop.max_iterations", &n.to_string())
    }

    /// Sets an environment variable for the spawned process (and its children).
    ///
    /// Useful for steering a backend wrapper, e.g. `MOCK_FIXTURE_PATH`.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Resolves the program + leading args for the configured binary.
    fn program_and_prefix(&self) -> (String, Vec<String>) {
        match &self.bin {
            AutoloopBin::PathLookup => ("autoloop".to_string(), Vec::new()),
            AutoloopBin::Node(path) => (
                "node".to_string(),
                vec![path.to_string_lossy().into_owned()],
            ),
            AutoloopBin::Explicit(path) => (path.to_string_lossy().into_owned(), Vec::new()),
        }
    }

    /// Assembles the full argv (excluding the program itself).
    ///
    /// Layout: `[<node bin>?] run <preset> [-b <backend>] [--set kv]... [--events path] <prompt>`.
    ///
    /// The positional prompt must remain last because autoloop parses it as a
    /// variadic argument; options placed after it can be consumed as prompt text.
    fn build_args(&self) -> Vec<String> {
        let (_program, mut args) = self.program_and_prefix();
        args.push("run".to_string());
        args.push(self.preset_dir.to_string_lossy().into_owned());

        if let Some(backend) = &self.backend {
            args.push("-b".to_string());
            // CRITICAL: backend value is exactly one argv element.
            args.push(backend.clone());
        }

        for kv in &self.set_overrides {
            args.push("--set".to_string());
            args.push(kv.clone());
        }

        if let Some(events) = &self.events_path {
            args.push("--events".to_string());
            args.push(events.to_string_lossy().into_owned());
        }

        args.push(self.prompt.clone());
        args
    }

    /// A human-readable representation of the command, for error messages.
    fn command_display(&self) -> String {
        let (program, _) = self.program_and_prefix();
        let args = self.build_args();
        let mut parts = vec![program];
        parts.extend(args);
        parts.join(" ")
    }

    /// Spawns `autoloop run`, waits for it to finish, and parses the summary.
    ///
    /// # Errors
    ///
    /// - [`AutoloopRunError::Spawn`] if the process cannot be launched.
    /// - [`AutoloopRunError::NonZeroExit`] if autoloop exits non-zero.
    /// - [`AutoloopRunError::UnparseableSummary`] if the summary block is absent
    ///   or malformed.
    pub fn run(&self) -> Result<AutoloopRunSummary, AutoloopRunError> {
        let child = self.spawn()?;
        self.wait_with_summary(child)
    }

    /// Spawns `autoloop run` with piped stdout/stderr and returns the live
    /// [`Child`] handle without waiting.
    ///
    /// This is the interruptible counterpart to [`Self::run`]: the caller owns
    /// the [`Child`] and can `kill()` it on Ctrl+C, then drop it (or hand it to
    /// [`Self::wait_with_summary`] for the success path). stdout/stderr are
    /// piped (NOT inherited) so a ratatui TUI driving the parent's tty is never
    /// corrupted by the subprocess's output.
    ///
    /// # Errors
    ///
    /// - [`AutoloopRunError::Spawn`] if the process cannot be launched.
    pub fn spawn(&self) -> Result<Child, AutoloopRunError> {
        let (program, _) = self.program_and_prefix();
        let args = self.build_args();

        let mut cmd = Command::new(&program);
        cmd.args(&args)
            .current_dir(&self.working_dir)
            .envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Make the child its own process-group leader so the caller can kill the
        // whole tree (autoloop + backend agent) with killpg. Only when requested
        // (the TUI path) — headless leaves the child in ralph's group.
        #[cfg(unix)]
        if self.own_process_group {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        cmd.spawn().map_err(|source| AutoloopRunError::Spawn {
            command: self.command_display(),
            source,
        })
    }

    /// Waits for a [`Child`] spawned by [`Self::spawn`] to finish, then parses
    /// the `autoloops summary` block from its captured stdout.
    ///
    /// Mirrors the post-wait handling of [`Self::run`] exactly, so the two share
    /// one success/error contract.
    ///
    /// # Errors
    ///
    /// - [`AutoloopRunError::Spawn`] if the output cannot be collected.
    /// - [`AutoloopRunError::NonZeroExit`] if autoloop exited non-zero.
    /// - [`AutoloopRunError::UnparseableSummary`] if the summary block is absent
    ///   or malformed.
    pub fn wait_with_summary(&self, child: Child) -> Result<AutoloopRunSummary, AutoloopRunError> {
        let output = child
            .wait_with_output()
            .map_err(|source| AutoloopRunError::Spawn {
                command: self.command_display(),
                source,
            })?;

        self.summary_from_output(output.status, &output.stdout, &output.stderr)
    }

    /// Waits for a spawned child while forwarding each stderr line to `on_line`.
    ///
    /// The callback runs on a dedicated blocking reader thread so stderr is
    /// drained live without risking a full child pipe. stdout remains captured
    /// and is used only for summary parsing. The trailing stderr bytes are kept
    /// for [`AutoloopRunError::NonZeroExit`], preserving [`Self::run`]'s error
    /// contract while allowing a headless frontend to surface verbose logs.
    pub fn wait_with_summary_streaming_stderr<F>(
        &self,
        mut child: Child,
        on_line: F,
    ) -> Result<AutoloopRunSummary, AutoloopRunError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let stderr = child.stderr.take().ok_or_else(|| AutoloopRunError::Spawn {
            command: self.command_display(),
            source: std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "spawned autoloop child has no piped stderr",
            ),
        })?;
        let stderr_reader = std::thread::spawn(move || read_stderr(stderr, on_line));

        // With stderr taken by the reader thread, wait_with_output drains only
        // stdout. Both pipes are therefore consumed concurrently.
        let output_result = child
            .wait_with_output()
            .map_err(|source| AutoloopRunError::Spawn {
                command: self.command_display(),
                source,
            });
        let stderr_tail = stderr_reader
            .join()
            .map_err(|_| AutoloopRunError::Spawn {
                command: self.command_display(),
                source: std::io::Error::other("autoloop stderr reader thread panicked"),
            })?
            .map_err(|source| AutoloopRunError::Spawn {
                command: self.command_display(),
                source,
            })?;
        let output = output_result?;

        self.summary_from_output(output.status, &output.stdout, &stderr_tail)
    }

    fn summary_from_output(
        &self,
        status: std::process::ExitStatus,
        stdout: &[u8],
        stderr: &[u8],
    ) -> Result<AutoloopRunSummary, AutoloopRunError> {
        if !status.success() {
            return Err(AutoloopRunError::NonZeroExit {
                code: status.code(),
                stderr_tail: stderr_tail(stderr),
            });
        }

        let stdout = String::from_utf8_lossy(stdout);
        parse_summary(&stdout).ok_or(AutoloopRunError::UnparseableSummary)
    }
}

/// Drains stderr line-by-line, forwarding text while retaining only its tail.
fn read_stderr<R, F>(stderr: R, mut on_line: F) -> std::io::Result<Vec<u8>>
where
    R: Read,
    F: FnMut(&str),
{
    let mut reader = BufReader::new(stderr);
    let mut line = Vec::new();
    let mut tail = Vec::new();

    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }

        let text = String::from_utf8_lossy(&line);
        on_line(text.trim_end_matches(['\r', '\n']));
        append_stderr_tail(&mut tail, &line);
    }

    Ok(tail)
}

fn append_stderr_tail(tail: &mut Vec<u8>, bytes: &[u8]) {
    if bytes.len() >= STDERR_TAIL_BYTES {
        tail.clear();
        tail.extend_from_slice(&bytes[bytes.len() - STDERR_TAIL_BYTES..]);
        return;
    }

    let overflow = tail
        .len()
        .saturating_add(bytes.len())
        .saturating_sub(STDERR_TAIL_BYTES);
    if overflow > 0 {
        tail.drain(..overflow);
    }
    tail.extend_from_slice(bytes);
}

/// Returns a UTF-8 tail of `bytes`, at most [`STDERR_TAIL_BYTES`] long.
fn stderr_tail(bytes: &[u8]) -> String {
    let start = bytes.len().saturating_sub(STDERR_TAIL_BYTES);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

/// Parses an `autoloops summary` block out of arbitrary stdout.
///
/// Scans for the `autoloops summary` header followed by a `===` separator line,
/// then reads `key: value` lines for the known fields. Tolerant of surrounding
/// log noise and extra whitespace. Returns `None` if the header/separator is
/// missing or any required field is absent or malformed.
pub fn parse_summary(stdout: &str) -> Option<AutoloopRunSummary> {
    let lines: Vec<&str> = stdout.lines().collect();

    // Find the header line, then require a `===` separator shortly after.
    let header_idx = lines
        .iter()
        .position(|l| l.trim().eq_ignore_ascii_case("autoloops summary"))?;

    // The separator is the next non-empty line and must be a run of `=`.
    let mut sep_idx = None;
    for (offset, line) in lines.iter().enumerate().skip(header_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.is_empty() && trimmed.chars().all(|c| c == '=') {
            sep_idx = Some(offset);
        }
        break;
    }
    let start = sep_idx? + 1;

    let mut run_id = None;
    let mut iterations = None;
    let mut stop_reason = None;
    let mut cost_usd = None;
    let mut journal = None;
    let mut memory = None;

    for line in &lines[start..] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            // A non key:value line after the fields ends the block.
            break;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "run_id" => run_id = Some(value.to_string()),
            "iterations" => iterations = value.parse::<u32>().ok(),
            "stop_reason" => stop_reason = Some(value.to_string()),
            "cost_usd" => cost_usd = value.parse::<f64>().ok(),
            "journal" => journal = Some(PathBuf::from(value)),
            "memory" => memory = Some(PathBuf::from(value)),
            _ => {}
        }
    }

    Some(AutoloopRunSummary {
        run_id: run_id?,
        iterations: iterations?,
        stop_reason: stop_reason?,
        // Older autoloop builds omit cost_usd; default to 0.0 rather than fail.
        cost_usd: cost_usd.unwrap_or(0.0),
        journal: journal?,
        memory: memory?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
autoloops summary
===================
run_id: run-abc123
iterations: 3
stop_reason: completed
journal: /tmp/work/.autoloop/journal.jsonl
memory: /tmp/work/.autoloop/memory.jsonl
";

    #[test]
    fn stderr_reader_forwards_lines_and_retains_bounded_tail() {
        let long_line = format!("{}\n", "x".repeat(STDERR_TAIL_BYTES));
        let input = format!("first\r\n{long_line}last-without-newline");
        let mut lines = Vec::new();

        let tail = read_stderr(input.as_bytes(), |line| lines.push(line.to_string()))
            .expect("stderr should be readable");

        assert_eq!(
            lines,
            vec![
                "first".to_string(),
                "x".repeat(STDERR_TAIL_BYTES),
                "last-without-newline".to_string(),
            ]
        );
        assert_eq!(tail.len(), STDERR_TAIL_BYTES);
        assert!(String::from_utf8_lossy(&tail).ends_with("last-without-newline"));
        assert!(!String::from_utf8_lossy(&tail).contains("first"));
    }

    #[test]
    fn parses_clean_summary_block() {
        let s = parse_summary(SAMPLE).expect("must parse");
        assert_eq!(s.run_id, "run-abc123");
        assert_eq!(s.iterations, 3);
        assert_eq!(s.stop_reason, "completed");
        assert_eq!(
            s.journal,
            PathBuf::from("/tmp/work/.autoloop/journal.jsonl")
        );
        assert_eq!(s.memory, PathBuf::from("/tmp/work/.autoloop/memory.jsonl"));
        // No cost_usd line (older autoloop / no usage) defaults to 0.0.
        assert!(s.cost_usd.abs() < f64::EPSILON);
    }

    #[test]
    fn parses_cumulative_cost_usd_when_present() {
        let block = "\
autoloops summary
===================
run_id: run-cost
iterations: 2
stop_reason: completed
cost_usd: 0.080000
journal: /j/journal.jsonl
memory: /m/memory.jsonl
";
        let s = parse_summary(block).expect("must parse with cost");
        assert!((s.cost_usd - 0.08).abs() < f64::EPSILON);
        assert_eq!(s.iterations, 2);
    }

    #[test]
    fn parses_summary_amid_log_noise() {
        let noisy = format!(
            "2026-06-23 starting up\nsome log line\n[backend] chatter\n{SAMPLE}\ntrailing log line\n"
        );
        let s = parse_summary(&noisy).expect("must parse amid noise");
        assert_eq!(s.run_id, "run-abc123");
        assert_eq!(s.iterations, 3);
        assert_eq!(s.stop_reason, "completed");
    }

    #[test]
    fn tolerates_extra_whitespace_and_field_order() {
        let block = "\
   autoloops summary
=========
stop_reason:   stalled
run_id:    xyz
memory:  /a/b/memory.jsonl
iterations:    12
journal:   /a/b/journal.jsonl
";
        let s = parse_summary(block).expect("must parse reordered");
        assert_eq!(s.run_id, "xyz");
        assert_eq!(s.iterations, 12);
        assert_eq!(s.stop_reason, "stalled");
        assert_eq!(s.journal, PathBuf::from("/a/b/journal.jsonl"));
        assert_eq!(s.memory, PathBuf::from("/a/b/memory.jsonl"));
    }

    #[test]
    fn returns_none_on_garbage() {
        assert!(parse_summary("").is_none());
        assert!(parse_summary("totally unrelated output\nwith no summary").is_none());
    }

    #[test]
    fn returns_none_when_header_present_but_no_separator() {
        let block = "autoloops summary\nrun_id: x\niterations: 1\nstop_reason: done\njournal: /j\nmemory: /m\n";
        assert!(parse_summary(block).is_none());
    }

    #[test]
    fn returns_none_when_iterations_not_a_number() {
        let block = "\
autoloops summary
===
run_id: x
iterations: not-a-number
stop_reason: done
journal: /j
memory: /m
";
        assert!(parse_summary(block).is_none());
    }

    #[test]
    fn returns_none_when_field_missing() {
        let block = "\
autoloops summary
===
run_id: x
iterations: 1
stop_reason: done
journal: /j
";
        // memory missing
        assert!(parse_summary(block).is_none());
    }

    #[test]
    fn build_args_places_all_options_before_prompt() {
        let runner = AutoloopRunner::new("/presets/autocode", "do the thing", "/work")
            .backend("node x.js")
            .max_iterations(3)
            .set_override("event_loop.max_runtime", "12000")
            .events_path("/tmp/events.jsonl");

        assert_eq!(
            runner.build_args(),
            vec![
                "run",
                "/presets/autocode",
                "-b",
                // The multi-word backend remains exactly one argv element.
                "node x.js",
                "--set",
                "event_loop.max_iterations=3",
                "--set",
                "event_loop.max_runtime=12000",
                "--events",
                "/tmp/events.jsonl",
                // Prompt is last so variadic prompt parsing cannot absorb options.
                "do the thing",
            ]
        );
    }

    #[test]
    fn node_bin_prepends_script_path() {
        let runner = AutoloopRunner::new("/p", "x", "/w")
            .bin(AutoloopBin::Node(PathBuf::from("/checkout/bin/autoloop")));
        let (program, _) = runner.program_and_prefix();
        assert_eq!(program, "node");
        let args = runner.build_args();
        assert_eq!(args, vec!["/checkout/bin/autoloop", "run", "/p", "x"]);
    }

    #[test]
    fn set_overrides_preserve_order() {
        let runner = AutoloopRunner::new("/p", "x", "/w")
            .set_override("a.b", "1")
            .set_override("c.d", "2");
        let args = runner.build_args();
        let first = args.iter().position(|a| a == "a.b=1").unwrap();
        let second = args.iter().position(|a| a == "c.d=2").unwrap();
        assert!(first < second);
    }

    /// Opt-in smoke test that drives the real autoloop binary against the
    /// shipped `autocode` preset using the deterministic mock backend. Skips
    /// gracefully when node, the autoloop checkout, or the preset are absent so
    /// CI on a bare clone still passes.
    #[test]
    fn runs_real_autoloop_with_mock_backend_when_available() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command as StdCommand;

        // Locate the autoloop checkout: AUTOLOOP_ROOT env override, else search
        // upward for a sibling `autoloop` dir. Robust from both the main
        // checkout and from git worktrees (a first-class Ralph workflow), where
        // a fixed relative depth would resolve to a nonexistent path and the
        // smoke test would silently skip.
        let autoloop_root = match find_autoloop_root() {
            Some(r) => r,
            None => {
                eprintln!(
                    "skip: autoloop checkout not found (set AUTOLOOP_ROOT or place a sibling 'autoloop' dir with bin/autoloop)"
                );
                return;
            }
        };
        let bin = autoloop_root.join("bin/autoloop");
        let preset = autoloop_root.join("packages/presets/presets/autocode");
        let mock = autoloop_root.join("dist/testing/mock-backend.js");
        let fixture = autoloop_root.join("test/fixtures/backend/routed-event-and-promise.json");

        if which("node").is_none() {
            eprintln!("skip: node not on PATH");
            return;
        }
        if !bin.is_file() {
            eprintln!("skip: {} not present", bin.display());
            return;
        }
        if !preset.is_dir() {
            eprintln!("skip: {} not present", preset.display());
            return;
        }
        if !mock.is_file() {
            eprintln!("skip: {} not present", mock.display());
            return;
        }
        if !fixture.is_file() {
            eprintln!("skip: {} not present", fixture.display());
            return;
        }

        // Temp git repo as the working dir.
        let tmp = tempfile::tempdir().expect("tempdir");
        let work = tmp.path();
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "test"],
        ] {
            let ok = StdCommand::new("git")
                .args(&args)
                .current_dir(work)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !ok {
                eprintln!("skip: git {args:?} failed");
                return;
            }
        }
        fs::write(work.join("index.html"), "<p>Hello</p>\n").expect("write index");
        let _ = StdCommand::new("git")
            .args(["add", "."])
            .current_dir(work)
            .status();
        let _ = StdCommand::new("git")
            .args(["commit", "-q", "-m", "init"])
            .current_dir(work)
            .status();

        // Single-token wrapper script pointing at the mock backend.
        let wrapper = work.join("mock-wrapper.sh");
        fs::write(
            &wrapper,
            format!("#!/usr/bin/env bash\nexec node {} \"$@\"\n", mock.display()),
        )
        .expect("write wrapper");
        let mut perms = fs::metadata(&wrapper).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&wrapper, perms).unwrap();

        // Point the mock at the fixture via the child's environment (inherited
        // by the wrapper + node grandchild). Avoids mutating process-global env.
        let runner = AutoloopRunner::new(&preset, "smoke prompt", work)
            .bin(AutoloopBin::Node(bin.clone()))
            .backend(wrapper.to_string_lossy().into_owned())
            .env("MOCK_FIXTURE_PATH", fixture.to_string_lossy().into_owned())
            .max_iterations(3);

        match runner.run() {
            Ok(summary) => {
                assert!(
                    !summary.stop_reason.is_empty(),
                    "stop_reason should be populated"
                );
                assert!(!summary.run_id.is_empty(), "run_id should be populated");
                eprintln!(
                    "autoloop smoke ok: run_id={} iterations={} stop_reason={}",
                    summary.run_id, summary.iterations, summary.stop_reason
                );
            }
            Err(e) => {
                // A non-zero exit or unparseable summary is a real failure here
                // since all preconditions were met.
                panic!("autoloop run failed despite available environment: {e}");
            }
        }
    }

    /// Locate the autoloop checkout for the opt-in smoke test. Prefers the
    /// `AUTOLOOP_ROOT` env var, else searches upward from the crate manifest for
    /// a sibling `autoloop/` containing `bin/autoloop`. Works from the main
    /// checkout and from git worktrees alike.
    #[cfg(test)]
    fn find_autoloop_root() -> Option<PathBuf> {
        if let Some(root) = std::env::var_os("AUTOLOOP_ROOT") {
            let p = PathBuf::from(root);
            if p.join("bin/autoloop").is_file() {
                return Some(p);
            }
        }
        for ancestor in std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors() {
            let candidate = ancestor.join("autoloop");
            if candidate.join("bin/autoloop").is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Minimal `PATH` lookup for an executable, test-only.
    #[cfg(test)]
    fn which(program: &str) -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path).find_map(|dir| {
            let candidate = dir.join(program);
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
    }
}
