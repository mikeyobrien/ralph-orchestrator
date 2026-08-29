//! v3 cutover: drive the autoloop runtime as ralph's orchestration engine.
//!
//! When `core.engine = "autoloop"`, `ralph run` spawns `autoloop run` as a
//! subprocess via [`AutoloopRunner`] instead of the in-house event loop,
//! consumes its structured `--events` LoopEvent stream, and maps the terminal
//! result onto ralph's [`TerminationReason`]. This is the thin-layer engine swap
//! at the heart of v3: autoloop owns loop execution; ralph coordinates.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use ralph_adapters::{
    AutoloopBin, AutoloopEvent, AutoloopEventTailer, AutoloopRunError, AutoloopRunSummary,
    AutoloopRunner, parse_events, parse_events_strict,
};
use ralph_core::{
    EventLoopConfig, LoopContext, RalphConfig, RunStats, TaskStore, TerminationReason,
    engine_state::{
        engine_config_overrides, engine_env, engine_journal_path, engine_memory_path,
        persist_current_run_id, prepare_engine_state_root, read_current_run_id,
    },
    sanitize_tui_inline_text,
};

use crate::rpc_events::RpcEventMapper;

use crate::completion_coord::{coordinate_completion, mark_merge_run_started};
use crate::display::Palette;

// Keep this test guard synchronized with autoloop's canonical
// packages/harness/src/types.ts STOP_REASONS list.
#[cfg(test)]
const KNOWN_STOP_REASONS: [&str; 25] = [
    "completed",
    "completion_event",
    "completion_promise",
    "max_iterations",
    "max_runtime",
    "cost_budget",
    "stalled",
    "error",
    "backend_failed",
    "backend_timeout",
    "verdict_takeover",
    "verdict_exit",
    "interrupted",
    "auth_failed",
    "quota_exhausted",
    "rate_limited",
    "transient_error",
    "review_unknown",
    "premature_quit",
    "suspended",
    "verdict_unknown",
    "completion_held",
    "parallel_wave_timeout",
    "parallel_wave_failed",
    "parallel_wave_invalid",
];

fn engine_stop_error(reason: &str) -> TerminationReason {
    TerminationReason::EngineError {
        detail: Some(reason.to_string()),
    }
}

/// Map an autoloop `stopReason` onto ralph's [`TerminationReason`].
fn map_stop_reason(reason: &str) -> TerminationReason {
    match reason {
        "completed" | "completion_event" | "completion_promise" | "verdict_exit" => {
            TerminationReason::CompletionPromise
        }
        "max_iterations" => TerminationReason::MaxIterations,
        "max_runtime" => TerminationReason::MaxRuntime,
        "cost_budget" => TerminationReason::MaxCost,
        "stalled" => TerminationReason::LoopStale,
        "interrupted" => TerminationReason::Interrupted,
        "suspended" => TerminationReason::Suspended,
        "completion_held" => TerminationReason::CompletionHeld,
        "error"
        | "backend_failed"
        | "backend_timeout"
        | "verdict_takeover"
        | "auth_failed"
        | "quota_exhausted"
        | "rate_limited"
        | "transient_error"
        | "review_unknown"
        | "premature_quit"
        | "verdict_unknown"
        | "parallel_wave_timeout"
        | "parallel_wave_failed"
        | "parallel_wave_invalid" => engine_stop_error(reason),
        _ => TerminationReason::EngineError {
            detail: Some("unknown engine stop reason".to_string()),
        },
    }
}

#[derive(Debug, PartialEq)]
struct FailedRunRecovery {
    reason: TerminationReason,
    iterations: u32,
    cost_usd: f64,
}

/// Recover an engine-provided terminal result after the subprocess exits non-zero.
///
/// A completion reason from a failed process is deliberately rejected: it must
/// not trigger completion or merge coordination when the child itself crashed.
fn recover_failed_run(events: &[AutoloopEvent]) -> Option<FailedRunRecovery> {
    let result = events.iter().rev().find_map(AutoloopEvent::run_result)?;
    let reason = map_stop_reason(&result.stop_reason);
    if reason == TerminationReason::CompletionPromise {
        return None;
    }

    Some(FailedRunRecovery {
        reason,
        iterations: result.iterations,
        cost_usd: result.cost_usd,
    })
}

fn read_events(path: &Path) -> Vec<AutoloopEvent> {
    std::fs::read_to_string(path)
        .map(|content| parse_events(&content))
        .unwrap_or_default()
}

fn validate_success_summary(summary: &AutoloopRunSummary, engine_root: &Path) -> Result<()> {
    let expected_journal = engine_journal_path(engine_root);
    let expected_memory = engine_memory_path(engine_root);
    if summary.journal != expected_journal || summary.memory != expected_memory {
        bail!("Autoloop summary did not report the expected Ralph-owned state files");
    }

    for path in [&expected_journal, &expected_memory] {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("Autoloop summary state file is an unsafe symlink");
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("Ralph could not validate an Autoloop summary state file"),
        }
    }

    Ok(())
}

/// Normalized result of driving the autoloop subprocess.
///
/// Keeping failures intact lets the completion path coordinate a crash before
/// returning the original diagnostic to the CLI.
#[derive(Debug)]
enum AutoloopOutcome {
    Completed(AutoloopRunSummary),
    Stopped,
    Failed(anyhow::Error),
}

/// Interpret a subprocess result using whether Ralph initiated a process-group
/// kill. autoloop deliberately re-raises SIGTERM/SIGKILL, which is represented
/// either as `128 + signal` or as a signaled status without a numeric exit code.
/// Those statuses are a user stop only after Ralph's own kill request; the same
/// status from an externally killed child remains a crash.
fn interpret_autoloop_result(
    result: Result<AutoloopRunSummary>,
    killed_by_ralph: bool,
) -> AutoloopOutcome {
    match result {
        Ok(summary) => AutoloopOutcome::Completed(summary),
        Err(error) if killed_by_ralph && is_kill_exit(&error) => AutoloopOutcome::Stopped,
        Err(error) => AutoloopOutcome::Failed(error),
    }
}

fn is_kill_exit(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<AutoloopRunError>(),
        Some(AutoloopRunError::NonZeroExit {
            code: Some(137 | 143) | None,
            ..
        })
    )
}

/// Resolve `p` against `workspace` when relative.
fn resolve(workspace: &Path, p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        path
    } else {
        workspace.join(path)
    }
}

/// Report Ralph tasks that could not participate in autoloop's completion gate.
///
/// This is deliberately observation only. autoloop remains the engine and owns
/// completion judgment using its canonical task store.
fn warn_on_open_ralph_tasks(tasks_path: &Path, loop_id: &str) {
    match TaskStore::load(tasks_path) {
        Ok(store) => {
            let open_count = store
                .open()
                .into_iter()
                .filter(|task| task.loop_id.as_deref() == Some(loop_id))
                .count();
            if open_count > 0 {
                eprintln!(
                    "WARNING: Loop completed with {open_count} open Ralph task(s) \
                     (loop_id={loop_id}). Ralph tasks use a separate task store and did not \
                     participate in loop completion."
                );
            }
        }
        Err(_) => eprintln!(
            "WARNING: Loop completed, but Ralph could not inspect its separate task store. \
             This observation failure does not change loop completion."
        ),
    }
}

/// Resolve the run identity and publish it for task ownership before startup.
///
/// Worktree loops keep the identity assigned when their context was created.
/// Primary fresh runs always replace a stale marker with a new identity, while
/// continue runs reuse either their explicit ID or the current marker.
fn prepare_loop_identity(
    context: &LoopContext,
    continue_mode: bool,
    explicit_loop_id: Option<&str>,
) -> Result<String> {
    let loop_id = if let Some(loop_id) = context.loop_id() {
        loop_id.to_string()
    } else if continue_mode {
        if let Some(loop_id) = explicit_loop_id {
            let loop_id = loop_id.trim();
            if loop_id.is_empty() {
                bail!("Cannot continue: --loop-id must not be blank");
            }
            loop_id.to_string()
        } else {
            let marker = context.ralph_dir().join("current-loop-id");
            let loop_id = std::fs::read_to_string(&marker).context(
                "Cannot continue: the current-loop-id marker is unavailable; pass --loop-id or start a fresh run",
            )?;
            let loop_id = loop_id.trim();
            if loop_id.is_empty() {
                bail!(
                    "Cannot continue: the current-loop-id marker is blank; pass --loop-id or start a fresh run"
                );
            }
            loop_id.to_string()
        }
    } else {
        format!("primary-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S-%f"))
    };

    let marker = context.ralph_dir().join("current-loop-id");
    std::fs::create_dir_all(context.ralph_dir()).context("creating the Ralph state directory")?;
    std::fs::write(&marker, &loop_id).context("writing the current loop ID marker")?;
    tracing::debug!(loop_id = %loop_id, "wrote current loop ID marker");

    Ok(loop_id)
}

/// Resolve the prompt passed to the autoloop subprocess.
///
/// Inline text takes precedence over a prompt file so callers that set
/// `event_loop.prompt` (including `ralph run -p`) cannot be shadowed by a stale
/// configured file path.
fn resolve_autoloop_prompt(workspace: &Path, event_loop: &EventLoopConfig) -> Result<String> {
    if let Some(prompt) = event_loop
        .prompt
        .as_ref()
        .filter(|prompt| !prompt.trim().is_empty())
    {
        return Ok(prompt.clone());
    }

    if event_loop.prompt_file.trim().is_empty() {
        return Ok(String::new());
    }

    let path = resolve(workspace, &event_loop.prompt_file);
    std::fs::read_to_string(&path).context("reading prompt file from the configured workspace")
}

/// How Ralph observes and continues an Autoloop engine process.
pub struct AutoloopLaunch {
    pub continue_mode: bool,
    pub native_resume: bool,
    pub use_colors: bool,
    pub tui: bool,
    pub rpc: bool,
}

/// Drive the configured autoloop preset as ralph's engine, returning the mapped
/// [`TerminationReason`].
///
/// After the subprocess terminates, runs the engine-agnostic completion
/// coordination ([`coordinate_completion`]) so parallel-loop bookkeeping
/// (merge queue, loop registry, landing, summary, history) matches the in-house
/// engine. `context` carries the loop identity; `None` means an ad-hoc run with
/// no merge-queue / registry participation.
pub async fn run_autoloop_engine(
    config: RalphConfig,
    autoloop_bin: AutoloopBin,
    context: Option<LoopContext>,
    auto_merge_override: Option<bool>,
    loop_id: Option<String>,
    launch: AutoloopLaunch,
) -> Result<TerminationReason> {
    let workspace = config.core.workspace_root.clone();
    let engine_state_root =
        prepare_engine_state_root(&workspace).context("preparing Ralph-owned Autoloop state")?;
    let identity_context = context
        .clone()
        .unwrap_or_else(|| LoopContext::primary(workspace.clone()));
    let loop_id =
        prepare_loop_identity(&identity_context, launch.continue_mode, loop_id.as_deref())?;

    if let Ok(merge_loop_id) = std::env::var("RALPH_MERGE_LOOP_ID") {
        let repo_root = context
            .as_ref()
            .map(|c| c.repo_root().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        mark_merge_run_started(&repo_root, &merge_loop_id, std::process::id());
    }

    // Use an explicit preset if configured; otherwise generate one from ralph's
    // native hats topology so existing ralph configs run on the autoloop engine
    // without a hand-authored preset.
    let (preset, explicit_preset) = match config.core.autoloop_preset.as_deref() {
        Some(p) => {
            let preset = resolve(&workspace, p);
            if !preset.join("autoloops.toml").is_file() {
                bail!(
                    "the configured Autoloop preset is unavailable or invalid; select a preset containing autoloops.toml"
                );
            }
            (preset, true)
        }
        None => {
            let preset = workspace.join(".ralph").join("autoloop-preset");
            crate::autoloop_preset_gen::generate_preset(&config, &preset).map_err(|_| {
                anyhow::anyhow!(
                    "could not generate Ralph's Autoloop preset; verify the workspace state directory is writable"
                )
            })?;
            tracing::debug!("engine=autoloop: generated preset from hats config");
            (preset, false)
        }
    };

    // Generated presets use Ralph's hat IDs as engine role IDs. Explicit
    // presets define their own role namespace, so their IDs are already the
    // best user-facing labels available.
    let role_display_names = if explicit_preset {
        HashMap::new()
    } else {
        config
            .hats
            .iter()
            .map(|(id, hat)| {
                let name = hat.name.trim();
                (
                    id.clone(),
                    if name.is_empty() {
                        id.clone()
                    } else {
                        name.to_string()
                    },
                )
            })
            .collect()
    };

    let prompt = if launch.native_resume {
        String::new()
    } else {
        resolve_autoloop_prompt(&workspace, &config.event_loop)?
    };

    // Structured event sink beneath the owned engine root — the preferred
    // observability channel. The root was prepared before any run state.
    let events_path = engine_state_root.join("events.ndjson");
    let _ = std::fs::remove_file(&events_path);

    tracing::debug!("engine=autoloop: driving the autoloop runtime as a subprocess");

    // Ralph owns the engine's runtime state beneath .ralph/. State-root and
    // exact-store environment overrides are both required: the root wins over
    // preset core.state_dir, while exact overrides win over a preset's explicit
    // journal/memory/tasks paths. Export absolute paths so child cwd handling
    // cannot re-anchor them.
    let mut runner = AutoloopRunner::new(preset, prompt.clone(), workspace.clone())
        .bin(autoloop_bin)
        .events_path(events_path.clone());
    if launch.native_resume {
        let run_id = read_current_run_id(&engine_state_root)
            .context("reading the persisted Autoloop run_id")?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Ralph has no persisted Autoloop run_id. Start a loop with `ralph run` before `ralph resume`."
                )
            })?;
        runner = runner.resume(run_id);
    }
    for (key, value) in engine_env(&engine_state_root) {
        runner = runner.env(key, value);
    }
    for (key, value) in engine_config_overrides(&engine_state_root) {
        runner = runner.set_override(key, &value);
    }
    if explicit_preset {
        for (key, value) in crate::autoloop_preset_gen::autoloop_budget_overrides(&config) {
            runner = runner.set_override(key, &value);
        }
    }

    let mut current_events_guard = None;
    let robot_service = if launch.tui {
        None
    } else if let Some(loop_context) = context.as_ref()
        && config.robot.enabled
        && loop_context.is_primary()
    {
        current_events_guard = Some(
            crate::autoloop_robot::CurrentEventsGuard::install(&workspace)
                .context("installing Autoloop current-events marker")?,
        );
        let service = crate::autoloop_robot::create_robot_service(&config, loop_context);
        if service.is_none() {
            current_events_guard = None;
        }
        service
    } else {
        None
    };

    let start = Instant::now();
    let outcome = if launch.rpc {
        match run_autoloop_rpc(
            runner,
            events_path.clone(),
            prompt.clone(),
            config.cli.backend.clone(),
            config.event_loop.max_iterations,
            role_display_names,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => AutoloopOutcome::Failed(error.context("autoloop RPC run failed")),
        }
    } else if launch.tui {
        // In-process TUI: render the autoloop run live by tailing its --events
        // file, concurrent with the subprocess. Resolves Ctrl+C by killing the
        // child (see run_autoloop_with_tui).
        match run_autoloop_with_tui(
            runner,
            events_path.clone(),
            workspace.clone(),
            engine_state_root.clone(),
            role_display_names,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => AutoloopOutcome::Failed(error.context("autoloop TUI run failed")),
        }
    } else if let Some(service) = robot_service {
        match run_autoloop_with_robot(
            runner,
            events_path.clone(),
            workspace.clone(),
            service,
            role_display_names,
            launch.use_colors,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => AutoloopOutcome::Failed(error.context("autoloop RObot run failed")),
        }
    } else {
        // Headless observes the same structured engine event stream as the TUI.
        // Keep the blocking child wait off the async runtime while a sibling
        // task tails and flushes live progress to stdout.
        match run_autoloop_headless(
            runner,
            events_path.clone(),
            role_display_names,
            launch.use_colors,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => AutoloopOutcome::Failed(error.context("autoloop headless run failed")),
        }
    };
    drop(current_events_guard);

    // Validate the complete structured stream before trusting even a successful
    // process and summary. Live readers remain lenient so they can keep
    // rendering, but any malformed nonblank record makes completion fail closed.
    let outcome = match std::fs::read_to_string(&events_path) {
        Ok(content) if parse_events_strict(&content).is_err() => AutoloopOutcome::Failed(
            anyhow::anyhow!("Autoloop produced a malformed structured event stream"),
        ),
        Ok(_) => outcome,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => outcome,
        Err(_) => AutoloopOutcome::Failed(anyhow::anyhow!(
            "Ralph could not validate the Autoloop structured event stream"
        )),
    };
    let outcome = match outcome {
        AutoloopOutcome::Completed(summary) => {
            match validate_success_summary(&summary, &engine_state_root) {
                Ok(()) => AutoloopOutcome::Completed(summary),
                Err(error) => AutoloopOutcome::Failed(error),
            }
        }
        outcome => outcome,
    };

    // Normalize every child execution result before coordinating so success,
    // user stop, spawn/wait failures, and non-zero exits all traverse the same
    // completion path exactly once. On failure, a valid engine terminal event
    // is authoritative when present. The subprocess diagnostic is still
    // returned after bookkeeping.
    let (reason, state, failure) = match outcome {
        AutoloopOutcome::Completed(summary) => {
            // A human /stop or /restart through the HITL bridge wins over the
            // engine's own terminal reason and is consumed exactly once.
            let reason = take_requested_termination(&workspace)
                .unwrap_or_else(|| map_stop_reason(&summary.stop_reason));
            if reason == TerminationReason::CompletionPromise {
                warn_on_open_ralph_tasks(&identity_context.tasks_path(), &loop_id);
            }
            let state = RunStats {
                iterations: summary.iterations,
                elapsed: start.elapsed(),
                cost_usd: summary.cost_usd,
            };
            (reason, state, None)
        }
        AutoloopOutcome::Stopped => (
            TerminationReason::Stopped,
            RunStats {
                elapsed: start.elapsed(),
                ..RunStats::default()
            },
            None,
        ),
        AutoloopOutcome::Failed(error) => {
            let recovery = recover_failed_run(&read_events(&events_path));
            let (reason, iterations, cost_usd) = recovery
                .map(|recovery| (recovery.reason, recovery.iterations, recovery.cost_usd))
                .unwrap_or((TerminationReason::ValidationFailure, 0, 0.0));
            (
                reason,
                RunStats {
                    iterations,
                    elapsed: start.elapsed(),
                    cost_usd,
                },
                Some(error),
            )
        }
    };

    persist_run_id_from_outcome(&engine_state_root, &events_path);

    let auto_merge = auto_merge_override.unwrap_or(config.features.auto_merge);

    coordinate_completion(
        &reason,
        &state,
        context.as_ref(),
        &config.core.scratchpad.path,
        &prompt,
        auto_merge,
        &loop_id,
        launch.use_colors,
        !launch.rpc,
    );

    match failure {
        Some(error) => Err(error),
        None => Ok(reason),
    }
}

#[derive(Debug)]
struct HeadlessPrintCtx {
    iteration: Option<u32>,
    max_iterations: Option<u32>,
    role_id: Option<String>,
    role_display_names: HashMap<String, String>,
    cumulative_cost_usd: Option<f64>,
    palette: Palette,
}

impl HeadlessPrintCtx {
    fn new(role_display_names: HashMap<String, String>, use_colors: bool) -> Self {
        Self {
            iteration: None,
            max_iterations: None,
            role_id: None,
            role_display_names,
            cumulative_cost_usd: None,
            palette: Palette::new(use_colors),
        }
    }

    /// Update routing context before rendering an event. A new iteration clears
    /// the previous role so a missing banner cannot mislabel the next start.
    fn observe(&mut self, event: &AutoloopEvent) {
        if let Some(iteration) = event.iteration {
            if self.iteration != Some(iteration) {
                self.role_id = None;
            }
            self.iteration = Some(iteration);
        }
        self.max_iterations = event.max_iterations.or(self.max_iterations);

        if let Some(role_id) = event
            .allowed_roles
            .as_ref()
            .and_then(|roles| roles.first())
            .map(|role| role.trim())
            .filter(|role| !role.is_empty() && !role.eq_ignore_ascii_case("unknown"))
        {
            self.role_id = Some(role_id.to_string());
        }
    }

    fn iteration_label(&self) -> String {
        let iteration = self.iteration.unwrap_or_default();
        match self.max_iterations {
            Some(max_iterations) => format!("{iteration}/{max_iterations}"),
            None => iteration.to_string(),
        }
    }

    fn role_name(&self) -> Option<&str> {
        let role_id = self.role_id.as_deref()?;
        Some(
            self.role_display_names
                .get(role_id)
                .map(String::as_str)
                .unwrap_or(role_id),
        )
    }

    fn role_segment(&self) -> String {
        let Some(role) = self.role_name() else {
            return String::new();
        };
        let p = self.palette;
        format!(" {}|{} {}{}{}", p.dim, p.reset, p.magenta, role, p.reset)
    }

    fn iteration_cost_segment(&mut self, cumulative_cost_usd: Option<f64>) -> String {
        let Some(cumulative) = cumulative_cost_usd else {
            return String::new();
        };
        let iteration = cumulative - self.cumulative_cost_usd.unwrap_or_default();
        self.cumulative_cost_usd = Some(cumulative);
        let p = self.palette;
        format!(
            " {}|{} {}cost ${iteration:.4} iteration, ${cumulative:.4} total{}",
            p.dim, p.reset, p.cyan, p.reset
        )
    }
}

/// Format one structured engine event in Ralph's headless voice.
///
/// The banner and footer are context-only: `iteration.start` opens a transition
/// and `progress` closes it with the routed topic. Terminal engine events stay
/// in the events file and journal; [`coordinate_completion`] renders Ralph's
/// sole user-facing ending.
fn format_headless_event(event: &AutoloopEvent, ctx: &mut HeadlessPrintCtx) -> Option<String> {
    ctx.observe(event);
    let p = ctx.palette;

    match event.kind.as_str() {
        "iteration.banner" | "iteration.footer" | "loop.finish" | "summary" => None,
        "iteration.start" => Some(format!(
            "{}{}Iteration {}{} started{}",
            p.bold,
            p.cyan,
            ctx.iteration_label(),
            p.reset,
            ctx.role_segment()
        )),
        "progress" => {
            let topic_segment = event
                .emitted_topic
                .as_deref()
                .map(|topic| {
                    format!(
                        " {}|{} emitted {}{}{}",
                        p.dim, p.reset, p.cyan, topic, p.reset
                    )
                })
                .unwrap_or_default();
            let role_segment = ctx.role_segment();
            let cost_segment = ctx.iteration_cost_segment(event.cost_usd);
            Some(format!(
                "{}{}Iteration {}{} finished{role_segment}{topic_segment}{cost_segment}",
                p.bold,
                p.green,
                ctx.iteration_label(),
                p.reset,
            ))
        }
        "ask.pending" => {
            let question = sanitize_tui_inline_text(event.question.as_deref().unwrap_or_default());
            Some(format!(
                "{}{}Iteration {}{} human input requested{} {}|{} {question}",
                p.bold,
                p.yellow,
                ctx.iteration_label(),
                p.reset,
                ctx.role_segment(),
                p.dim,
                p.reset
            ))
        }
        _ => None,
    }
}

fn print_headless_event(event: &AutoloopEvent, ctx: &mut HeadlessPrintCtx) {
    let Some(line) = format_headless_event(event, ctx) else {
        return;
    };
    println!("{line}");
    // A pipe is block-buffered; explicitly flush so bot daemons and callers
    // consuming stdout observe each progress line before the child exits.
    let _ = std::io::stdout().flush();
}

fn trace_engine_diagnostic() {
    tracing::debug!(
        target: "autoloop::engine",
        "Autoloop emitted diagnostic output"
    );
}

/// Run autoloop headlessly while live-tailing its structured event stream.
async fn run_autoloop_headless(
    runner: AutoloopRunner,
    events_path: PathBuf,
    role_display_names: HashMap<String, String>,
    use_colors: bool,
) -> Result<AutoloopOutcome> {
    use tokio::sync::watch;

    // Headless intentionally leaves autoloop in Ralph's process group. Unlike
    // the interactive TUI path, there is no local quit action requiring Ralph
    // to kill a separately-owned subprocess tree.
    let child = runner.spawn().context("spawning the autoloop subprocess")?;
    let (terminated_tx, mut terminated_rx) = watch::channel(false);

    let reader_handle = tokio::spawn(async move {
        let mut tailer = AutoloopEventTailer::new(events_path);
        let mut ctx = HeadlessPrintCtx::new(role_display_names, use_colors);
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(100));

        loop {
            tokio::select! {
                biased;

                _ = terminated_rx.changed() => {
                    if *terminated_rx.borrow() {
                        break;
                    }
                }
                _ = ticker.tick() => {
                    match tailer.poll() {
                        Ok(events) => {
                            for event in &events {
                                print_headless_event(event, &mut ctx);
                            }
                        }
                        Err(error) => {
                            tracing::debug!(%error, "headless autoloop event reader poll failed");
                        }
                    }
                }
            }
        }

        // autoloop writes terminal events immediately before exit. Drain once
        // after cancellation so those final structured facts are not lost.
        match tailer.poll() {
            Ok(events) => {
                for event in &events {
                    print_headless_event(event, &mut ctx);
                }
            }
            Err(error) => {
                tracing::debug!(%error, "headless autoloop event reader final drain failed");
            }
        }
    });

    let wait_handle = tokio::spawn(async move {
        let summary = tokio::task::spawn_blocking(move || {
            runner.wait_with_summary_streaming_stderr(child, trace_engine_diagnostic)
        })
        .await
        .context("autoloop wait task panicked")
        .and_then(|summary| summary.context("autoloop run failed"));
        let _ = terminated_tx.send(true);
        summary
    });

    let summary = wait_handle.await.context("autoloop wait join failed")?;
    reader_handle
        .await
        .context("headless autoloop event reader task failed")?;

    Ok(interpret_autoloop_result(summary, false))
}

fn persist_run_id_from_outcome(engine_state_root: &Path, events_path: &Path) {
    if let Some(run_id) = read_events(events_path)
        .iter()
        .rev()
        .find_map(|event| event.run_id.clone())
    {
        let _ = persist_current_run_id(engine_state_root, &run_id);
    }
}

/// Headless Autoloop run that serializes the same `--events` file as `RpcEvent` JSON lines.
async fn run_autoloop_rpc(
    runner: AutoloopRunner,
    events_path: PathBuf,
    prompt: String,
    backend: String,
    max_iterations: Option<u32>,
    role_display_names: HashMap<String, String>,
) -> Result<AutoloopOutcome> {
    use ralph_proto::json_rpc::emit_event_line;
    use tokio::sync::watch;

    let child = runner.spawn().context("spawning the autoloop subprocess")?;
    let (terminated_tx, mut terminated_rx) = watch::channel(false);

    let reader_handle = tokio::spawn(async move {
        let mut tailer = AutoloopEventTailer::new(events_path);
        let mut mapper = RpcEventMapper::new(prompt, backend, max_iterations, role_display_names);
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(100));

        let emit = |event: ralph_proto::json_rpc::RpcEvent| {
            print!("{}", emit_event_line(&event));
            let _ = std::io::stdout().flush();
        };

        loop {
            tokio::select! {
                biased;

                _ = terminated_rx.changed() => {
                    if *terminated_rx.borrow() {
                        break;
                    }
                }
                _ = ticker.tick() => {
                    match tailer.poll() {
                        Ok(events) => {
                            for event in &events {
                                for rpc_event in mapper.map(event) {
                                    emit(rpc_event);
                                }
                            }
                        }
                        Err(error) => {
                            tracing::debug!(%error, "RPC autoloop event reader poll failed");
                        }
                    }
                }
            }
        }

        match tailer.poll() {
            Ok(events) => {
                for event in &events {
                    for rpc_event in mapper.map(event) {
                        emit(rpc_event);
                    }
                }
            }
            Err(error) => {
                tracing::debug!(%error, "RPC autoloop event reader final drain failed");
            }
        }
    });

    let wait_handle = tokio::spawn(async move {
        let summary = tokio::task::spawn_blocking(move || {
            runner.wait_with_summary_streaming_stderr(child, trace_engine_diagnostic)
        })
        .await
        .context("autoloop wait task panicked")
        .and_then(|summary| summary.context("autoloop run failed"));
        let _ = terminated_tx.send(true);
        summary
    });

    let summary = wait_handle.await.context("autoloop wait join failed")?;
    reader_handle
        .await
        .context("RPC autoloop event reader task failed")?;

    Ok(interpret_autoloop_result(summary, false))
}

fn take_requested_termination(workspace: &Path) -> Option<TerminationReason> {
    let ralph_dir = workspace.join(".ralph");
    let restart = ralph_dir.join("restart-requested");
    if restart.exists() {
        let _ = std::fs::remove_file(restart);
        return Some(TerminationReason::RestartRequested);
    }
    let stop = ralph_dir.join("stop-requested");
    if stop.exists() {
        let _ = std::fs::remove_file(stop);
        return Some(TerminationReason::Stopped);
    }
    None
}

/// Run Autoloop headlessly while relaying RObot asks and guidance through its
/// file-backed control protocol. Also prints the same structured progress
/// lines as the plain headless path.
async fn run_autoloop_with_robot(
    runner: AutoloopRunner,
    events_path: PathBuf,
    workspace: PathBuf,
    service: Box<dyn ralph_proto::RobotService>,
    role_display_names: HashMap<String, String>,
    use_colors: bool,
) -> Result<AutoloopOutcome> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::watch;

    let runner = runner.own_process_group(true);
    let child = runner.spawn().context("spawning the autoloop subprocess")?;
    let done = Arc::new(AtomicBool::new(false));
    let robot_shutdown = service.shutdown_flag();
    let mut process_guard =
        AutoloopProcessGuard::new(child.id(), Arc::clone(&done), Arc::clone(&robot_shutdown));

    let (terminated_tx, mut terminated_rx) = watch::channel(false);
    let printer_events = events_path.clone();
    let printer_handle = tokio::spawn(async move {
        let mut tailer = AutoloopEventTailer::new(printer_events);
        let mut ctx = HeadlessPrintCtx::new(role_display_names, use_colors);
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(100));

        loop {
            tokio::select! {
                biased;
                _ = terminated_rx.changed() => {
                    if *terminated_rx.borrow() {
                        break;
                    }
                }
                _ = ticker.tick() => {
                    match tailer.poll() {
                        Ok(events) => {
                            for event in &events {
                                print_headless_event(event, &mut ctx);
                            }
                        }
                        Err(error) => {
                            tracing::debug!(%error, "RObot autoloop event reader poll failed");
                        }
                    }
                }
            }
        }

        match tailer.poll() {
            Ok(events) => {
                for event in &events {
                    print_headless_event(event, &mut ctx);
                }
            }
            Err(error) => {
                tracing::debug!(%error, "RObot autoloop event reader final drain failed");
            }
        }
    });

    let wait_runner = runner.clone();
    let mut wait_handle = tokio::task::spawn_blocking(move || {
        wait_runner.wait_with_summary_streaming_stderr(child, trace_engine_diagnostic)
    });

    let bridge_runner = runner.clone();
    let bridge_done = Arc::clone(&done);
    let mut bridge_handle = tokio::task::spawn_blocking(move || {
        crate::autoloop_robot::run_bridge(
            service,
            bridge_runner,
            events_path,
            workspace,
            bridge_done,
        )
    });

    tokio::select! {
        summary = &mut wait_handle => {
            process_guard.disarm();
            done.store(true, Ordering::Release);
            robot_shutdown.store(true, Ordering::Release);
            let _ = terminated_tx.send(true);
            let bridge_result = bridge_handle
                .await
                .context("Autoloop RObot bridge panicked")?;
            let _ = printer_handle.await;
            let summary = summary
                .context("autoloop wait task panicked")?
                .context("autoloop run failed")?;
            bridge_result?;
            Ok(interpret_autoloop_result(Ok(summary), false))
        }
        bridge = &mut bridge_handle => {
            process_guard.terminate();
            done.store(true, Ordering::Release);
            robot_shutdown.store(true, Ordering::Release);
            let _ = terminated_tx.send(true);
            let _ = printer_handle.await;
            let bridge_result = bridge.context("Autoloop RObot bridge panicked")?;
            let _ = wait_handle.await;
            bridge_result?;
            anyhow::bail!("Autoloop RObot bridge stopped before the subprocess")
        }
    }
}

struct AutoloopProcessGuard {
    pid: u32,
    armed: bool,
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    robot_shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl AutoloopProcessGuard {
    fn new(
        pid: u32,
        done: std::sync::Arc<std::sync::atomic::AtomicBool>,
        robot_shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            pid,
            armed: true,
            done,
            robot_shutdown,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    fn terminate(&mut self) {
        if self.armed {
            use std::sync::atomic::Ordering;

            self.done.store(true, Ordering::Release);
            self.robot_shutdown.store(true, Ordering::Release);
            kill_autoloop_group(self.pid);
            self.armed = false;
        }
    }
}

impl Drop for AutoloopProcessGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// Run the autoloop subprocess with the in-process live TUI.
///
/// The TUI renders inside this (parent) process, concurrent with the `autoloop
/// run` subprocess, fed by live-tailing the `--events` file. tokio is
/// multi-threaded (`#[tokio::main]`), so the blocking subprocess wait
/// (`spawn_blocking`), the async TUI render loop, and the async event-reader
/// poll task coexist. The subprocess's stdout/stderr are piped (see
/// `AutoloopRunner::spawn`), so they never corrupt the ratatui tty.
///
/// ## Ctrl+C behavior (FIX gap#2)
///
/// `AutoloopRunner` exposes [`AutoloopRunner::spawn`], so on Ctrl+C the TUI
/// signals via its interrupt channel and we **kill the autoloop child**
/// (SIGTERM, then SIGKILL) — a clean teardown, NOT a blank-terminal hang. The
/// blocking wait then returns the child's (killed) result and we fall through
/// to the shared post-run path. This is the proper refactor the design calls
/// for, not the `exit(130)` fallback.
async fn run_autoloop_with_tui(
    runner: AutoloopRunner,
    events_path: PathBuf,
    workspace: PathBuf,
    engine_state_root: PathBuf,
    role_display_names: HashMap<String, String>,
) -> Result<AutoloopOutcome> {
    use ralph_tui::Tui;
    use tokio::sync::watch;

    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    // Termination drives BOTH the TUI shutdown and the event reader's final
    // drain. App needs an interrupt sink (Ctrl+C), but we don't watch it — the
    // TUI returning (on q OR Ctrl+C) is what triggers the kill below.
    let (terminated_tx, terminated_rx) = watch::channel(false);
    let (interrupt_tx, _interrupt_rx) = watch::channel(false);

    let tui = Tui::new()
        .with_termination_signal(terminated_rx.clone())
        .with_interrupt_tx(interrupt_tx)
        .with_export_workspace_root(workspace.clone());
    let state = tui.state();

    // Mark the source so the footer suppresses guidance/steer affordances that
    // have no back-channel to the autoloop child (FIX gap#6).
    if let Ok(mut s) = state.lock() {
        s.autoloop_source = true;
    }

    // Live-tail the --events file into the TUI state until cancelled.
    let reader_handle = {
        let reader_state = Arc::clone(&state);
        let cancel_rx = terminated_rx.clone();
        tokio::spawn(async move {
            ralph_tui::run_autoloop_event_reader(
                events_path,
                workspace,
                engine_state_root,
                reader_state,
                cancel_rx,
                role_display_names,
            )
            .await;
        })
    };

    // Spawn autoloop as its OWN process-group leader (piped stdio) so that on a
    // user quit we can kill the whole tree (autoloop + its backend agent) and
    // not orphan the agent. Headless keeps the child in ralph's group.
    let runner = runner.own_process_group(true);
    let child = runner.spawn().context("spawning the autoloop subprocess")?;
    let child_pid = child.id();

    // Block on the subprocess in a worker thread, freeing the async runtime for
    // the TUI + reader. wait_with_summary mirrors run()'s success/error contract.
    // On natural exit set `completed` and signal the TUI to drop — this is what
    // unblocks `tui.run()` when the run finishes on its own.
    let completed = Arc::new(AtomicBool::new(false));
    let wait_handle = {
        let terminated_tx = terminated_tx.clone();
        let completed = Arc::clone(&completed);
        tokio::spawn(async move {
            let summary = tokio::task::spawn_blocking(move || runner.wait_with_summary(child))
                .await
                .context("autoloop wait task panicked")?;
            completed.store(true, Ordering::SeqCst);
            let _ = terminated_tx.send(true);
            summary.context("autoloop run failed")
        })
    };

    // Run the TUI render/input loop concurrently with the subprocess. It returns
    // on natural completion (terminated_tx) OR on q / Ctrl+C.
    let tui_result = tui.run().await;

    // If the subprocess is still running (user quit via q or Ctrl+C — neither
    // exits autoloop), stop the whole process group so the backend isn't
    // orphaned. No-op if the run already completed naturally.
    let killed_by_ralph = if completed.load(Ordering::SeqCst) {
        false
    } else {
        kill_autoloop_group(child_pid);
        true
    };
    // Ensure the reader does its final drain even if the TUI exited first (q).
    let _ = terminated_tx.send(true);

    // Collect the subprocess result, then tear down the auxiliary tasks.
    let summary = wait_handle.await.context("autoloop wait join failed")?;
    let _ = reader_handle.await;
    tui_result.context("TUI render loop failed")?;

    Ok(interpret_autoloop_result(summary, killed_by_ralph))
}

/// Stop the autoloop subprocess tree: SIGTERM the whole process group, then
/// escalate to SIGKILL after a short grace so autoloop and its backend agent can
/// exit cleanly (flush, release locks) first. `pid` is the group leader's pid
/// (the child was spawned with [`AutoloopRunner::own_process_group`]). Off Unix
/// this is a best-effort no-op. The blocking wait reaps the child once it exits.
fn kill_autoloop_group(pid: u32) {
    #[cfg(unix)]
    {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;
        let pgid = Pid::from_raw(pid as i32);
        let _ = killpg(pgid, Signal::SIGTERM);
        // Detached escalation: hard-kill the group if it ignores SIGTERM. A
        // SIGKILL to an already-dead group is ESRCH and harmless. Drop may
        // run off the async runtime (spawn_blocking), so fall back to a thread.
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let _ = killpg(pgid, Signal::SIGKILL);
            });
        } else {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(2));
                let _ = killpg(pgid, Signal::SIGKILL);
            });
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}

fn prepare_daemon_engine_state(
    workspace_root: &Path,
    resolve_engine: impl FnOnce() -> Result<AutoloopBin>,
) -> Result<AutoloopBin> {
    let autoloop_bin = resolve_engine()?;
    prepare_engine_state_root(workspace_root).context("preparing Ralph-owned Autoloop state")?;
    Ok(autoloop_bin)
}

/// Start a headless orchestration loop for the Telegram bot daemon.
///
/// This is the daemon's [`ralph_proto::StartLoopFn`] target: it loads config,
/// applies the supplied prompt, forces autonomous/headless mode, acquires the
/// primary loop lock, and drives the autoloop engine. When `RObot.enabled` is
/// set, the HITL bridge relays Autoloop `ask.pending` through Telegram/Web.
pub async fn start_loop(
    prompt: String,
    workspace_root: PathBuf,
    config_path: Option<PathBuf>,
) -> Result<TerminationReason> {
    use crate::{ConfigSource, load_config_with_overrides};

    // Load config from file or defaults.
    let config_source = config_path.unwrap_or_else(|| workspace_root.join("ralph.yml"));
    let sources = vec![ConfigSource::File(config_source)];
    let mut config = load_config_with_overrides(&sources)?;

    // Set workspace root to the provided path.
    config.core.workspace_root = workspace_root.clone();

    // Apply the prompt.
    config.event_loop.prompt = Some(prompt);
    config.event_loop.prompt_file = String::new();

    // Force autonomous headless mode (no TUI, no interactive).
    config.cli.default_mode = "autonomous".to_string();

    // Normalize and validate.
    config.normalize();
    let warnings = config
        .validate()
        .context("Configuration validation failed")?;
    for warning in &warnings {
        tracing::warn!("{}", warning);
    }

    // Auto-detect backend if needed.
    if config.cli.backend == "auto" {
        let priority = config.get_agent_priority();
        let detected = ralph_adapters::detect_backend(&priority, |backend| {
            config.adapter_settings(backend).enabled
        });
        match detected {
            Ok(backend) => {
                tracing::debug!("Auto-detected backend: {}", backend);
                config.cli.backend = backend;
            }
            Err(e) => return Err(anyhow::Error::new(e)),
        }
    }

    // Resolve once before creating state or acquiring the loop lock, then use
    // that exact binary for launch. Dependency failure must remain state-free.
    let autoloop_bin = prepare_daemon_engine_state(&workspace_root, || {
        crate::engine_provision::ensure_autoloop_with_provisioning(false, false)
    })?;

    // Ensure scratchpad directory exists.
    crate::ensure_scratchpad_directory(&config)?;

    // Acquire the loop lock (primary loop).
    let prompt_summary = config.event_loop.prompt.as_deref().unwrap_or("[daemon]");
    let prompt_summary = ralph_core::truncate_with_ellipsis(prompt_summary, 100);

    let _lock_guard = ralph_core::LoopLock::try_acquire(&workspace_root, &prompt_summary)
        .context("Failed to acquire loop lock — another loop may be running")?;

    let loop_context = ralph_core::LoopContext::primary(workspace_root);

    // Drive the loop headlessly via the autoloop engine (daemon: never a TUI).
    loop {
        let reason = run_autoloop_engine(
            config.clone(),
            autoloop_bin.clone(),
            Some(loop_context.clone()),
            None,
            None,
            AutoloopLaunch {
                continue_mode: false,
                native_resume: false,
                use_colors: false,
                tui: false,
                rpc: false,
            },
        )
        .await?;
        if reason == TerminationReason::RestartRequested {
            continue;
        }
        return Ok(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nonzero_exit(code: Option<i32>) -> anyhow::Error {
        anyhow::Error::new(AutoloopRunError::NonZeroExit {
            command: "autoloop (PATH lookup): run".to_string(),
            code,
        })
        .context("autoloop run failed")
    }

    fn test_summary() -> AutoloopRunSummary {
        AutoloopRunSummary {
            run_id: "run-test".to_string(),
            iterations: 3,
            stop_reason: "completed".to_string(),
            cost_usd: 0.25,
            journal: PathBuf::from("/tmp/journal.jsonl"),
            memory: PathBuf::from("/tmp/memory.jsonl"),
        }
    }

    #[test]
    fn headless_formatter_reports_hat_names_without_stale_or_unknown_roles() {
        let events = ralph_adapters::parse_events(concat!(
            r#"{"type":"iteration.banner","iteration":1,"maxIterations":4,"allowedRoles":["planner"]}"#,
            "\n",
            r#"{"type":"iteration.start","runId":"r1","iteration":1,"maxIterations":4}"#,
            "\n",
            r#"{"type":"progress","runId":"r1","iteration":1,"allowedRoles":["planner"],"emittedTopic":"plan.ready","outcome":"continue:routed_event","costUsd":0.1}"#,
            "\n",
            r#"{"type":"iteration.footer","iteration":1,"elapsedS":1.5}"#,
            "\n",
            // No banner for iteration 2: its start omits the hat instead of
            // showing the previous hat or inventing an "unknown" role.
            r#"{"type":"iteration.start","runId":"r1","iteration":2,"maxIterations":4}"#,
            "\n",
            r#"{"type":"progress","runId":"r1","iteration":2,"allowedRoles":["builder"],"emittedTopic":"build.done","outcome":"continue:routed_event","costUsd":0.25}"#,
            "\n",
            r#"{"type":"loop.finish","runId":"r1","iterations":2,"stopReason":"completed","costUsd":0.25}"#,
            "\n",
        ));
        let role_names = HashMap::from([
            ("planner".to_string(), "Planning Lead".to_string()),
            ("builder".to_string(), "Build Crew".to_string()),
        ]);
        let mut ctx = HeadlessPrintCtx::new(role_names, false);
        let lines: Vec<_> = events
            .iter()
            .filter_map(|event| format_headless_event(event, &mut ctx))
            .collect();

        assert_eq!(
            lines,
            vec![
                "Iteration 1/4 started | Planning Lead",
                "Iteration 1/4 finished | Planning Lead | emitted plan.ready | cost $0.1000 iteration, $0.1000 total",
                "Iteration 2/4 started",
                "Iteration 2/4 finished | Build Crew | emitted build.done | cost $0.1500 iteration, $0.2500 total",
            ]
        );
        assert!(lines.iter().all(|line| !line.contains("unknown")));
    }

    #[test]
    fn headless_formatter_sanitizes_human_ask() {
        let events = ralph_adapters::parse_events(concat!(
            r#"{"type":"iteration.banner","iteration":1,"maxIterations":3,"allowedRoles":["planner"]}"#,
            "\n",
            r#"{"type":"ask.pending","runId":"r1","iteration":1,"questionId":"q1","question":"line1\nline2\u0007"}"#,
            "\n",
        ));
        let role_names = HashMap::from([("planner".to_string(), "Planning Lead".to_string())]);
        let mut ctx = HeadlessPrintCtx::new(role_names, false);
        let lines: Vec<_> = events
            .iter()
            .filter_map(|event| format_headless_event(event, &mut ctx))
            .collect();

        assert_eq!(
            lines,
            vec!["Iteration 1/3 human input requested | Planning Lead | line1 line2"]
        );
    }

    #[test]
    fn fresh_primary_run_replaces_stale_loop_id_marker() {
        let workspace = tempfile::tempdir().unwrap();
        let context = LoopContext::primary(workspace.path().to_path_buf());
        std::fs::create_dir_all(context.ralph_dir()).unwrap();
        std::fs::write(context.ralph_dir().join("current-loop-id"), "stale-loop").unwrap();

        let loop_id = prepare_loop_identity(&context, false, None).unwrap();

        assert_ne!(loop_id, "stale-loop");
        assert!(loop_id.starts_with("primary-"));
        assert_eq!(
            std::fs::read_to_string(context.ralph_dir().join("current-loop-id")).unwrap(),
            loop_id
        );
    }

    #[test]
    fn implicit_continue_reuses_loop_id_marker() {
        let workspace = tempfile::tempdir().unwrap();
        let context = LoopContext::primary(workspace.path().to_path_buf());
        std::fs::create_dir_all(context.ralph_dir()).unwrap();
        std::fs::write(
            context.ralph_dir().join("current-loop-id"),
            " previous-loop \n",
        )
        .unwrap();

        let loop_id = prepare_loop_identity(&context, true, None).unwrap();

        assert_eq!(loop_id, "previous-loop");
        assert_eq!(
            std::fs::read_to_string(context.ralph_dir().join("current-loop-id")).unwrap(),
            "previous-loop"
        );
    }

    #[test]
    fn explicit_continue_loop_id_wins_over_marker() {
        let workspace = tempfile::tempdir().unwrap();
        let context = LoopContext::primary(workspace.path().to_path_buf());
        std::fs::create_dir_all(context.ralph_dir()).unwrap();
        std::fs::write(context.ralph_dir().join("current-loop-id"), "previous-loop").unwrap();

        let loop_id = prepare_loop_identity(&context, true, Some("chosen-loop")).unwrap();

        assert_eq!(loop_id, "chosen-loop");
        assert_eq!(
            std::fs::read_to_string(context.ralph_dir().join("current-loop-id")).unwrap(),
            "chosen-loop"
        );
    }

    #[test]
    fn implicit_continue_errors_when_loop_id_marker_is_missing() {
        let workspace = tempfile::tempdir().unwrap();
        let context = LoopContext::primary(workspace.path().to_path_buf());

        let error = prepare_loop_identity(&context, true, None).unwrap_err();

        assert!(error.to_string().contains("Cannot continue"));
        assert!(error.to_string().contains("current-loop-id"));
        assert!(error.to_string().contains("--loop-id"));
    }

    #[test]
    fn implicit_continue_errors_when_loop_id_marker_is_blank() {
        let workspace = tempfile::tempdir().unwrap();
        let context = LoopContext::primary(workspace.path().to_path_buf());
        std::fs::create_dir_all(context.ralph_dir()).unwrap();
        std::fs::write(context.ralph_dir().join("current-loop-id"), " \n").unwrap();

        let error = prepare_loop_identity(&context, true, None).unwrap_err();

        assert!(error.to_string().contains("Cannot continue"));
        assert!(error.to_string().contains("blank"));
        assert!(error.to_string().contains("--loop-id"));
    }

    #[test]
    fn worktree_run_keeps_context_loop_id() {
        let workspace = tempfile::tempdir().unwrap();
        let repo_root = tempfile::tempdir().unwrap();
        let context = LoopContext::worktree(
            "worktree-loop",
            workspace.path().to_path_buf(),
            repo_root.path().to_path_buf(),
        );

        let loop_id = prepare_loop_identity(&context, false, None).unwrap();

        assert_eq!(loop_id, "worktree-loop");
        assert_eq!(
            std::fs::read_to_string(context.ralph_dir().join("current-loop-id")).unwrap(),
            "worktree-loop"
        );
    }

    #[test]
    fn ralph_sigterm_kill_maps_to_stopped() {
        let outcome = interpret_autoloop_result(Err(nonzero_exit(Some(128 + 15))), true);

        assert!(matches!(outcome, AutoloopOutcome::Stopped));
    }

    #[test]
    fn ralph_sigkill_maps_to_stopped() {
        let outcome = interpret_autoloop_result(Err(nonzero_exit(Some(128 + 9))), true);

        assert!(matches!(outcome, AutoloopOutcome::Stopped));
    }

    #[test]
    fn ralph_signaled_status_without_exit_code_maps_to_stopped() {
        let outcome = interpret_autoloop_result(Err(nonzero_exit(None)), true);

        assert!(matches!(outcome, AutoloopOutcome::Stopped));
    }

    #[test]
    fn external_sigkill_exit_remains_failure() {
        let outcome = interpret_autoloop_result(Err(nonzero_exit(Some(128 + 9))), false);

        let AutoloopOutcome::Failed(error) = outcome else {
            panic!("external signal must remain a crash");
        };
        assert!(matches!(
            error.downcast_ref::<AutoloopRunError>(),
            Some(AutoloopRunError::NonZeroExit {
                code: Some(137),
                ..
            })
        ));
    }

    #[test]
    fn successful_summary_is_preserved() {
        let summary = test_summary();
        let outcome = interpret_autoloop_result(Ok(summary.clone()), false);

        let AutoloopOutcome::Completed(actual) = outcome else {
            panic!("successful run must retain its summary");
        };
        assert_eq!(actual, summary);
    }

    #[test]
    fn autoloop_prompt_prefers_inline_text_over_file() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("prompt.md"), "file prompt").unwrap();
        let event_loop = EventLoopConfig {
            prompt: Some("inline prompt".to_string()),
            prompt_file: "prompt.md".to_string(),
            ..EventLoopConfig::default()
        };

        let prompt = resolve_autoloop_prompt(workspace.path(), &event_loop).unwrap();

        assert_eq!(prompt, "inline prompt");
    }

    #[test]
    fn autoloop_prompt_reads_file_when_inline_text_is_absent() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("prompt.md"), "file prompt").unwrap();
        let event_loop = EventLoopConfig {
            prompt: None,
            prompt_file: "prompt.md".to_string(),
            ..EventLoopConfig::default()
        };

        let prompt = resolve_autoloop_prompt(workspace.path(), &event_loop).unwrap();

        assert_eq!(prompt, "file prompt");
    }

    #[test]
    fn autoloop_prompt_is_empty_when_no_source_is_configured() {
        let workspace = tempfile::tempdir().unwrap();
        let event_loop = EventLoopConfig {
            prompt: None,
            prompt_file: String::new(),
            ..EventLoopConfig::default()
        };

        let prompt = resolve_autoloop_prompt(workspace.path(), &event_loop).unwrap();

        assert!(prompt.is_empty());
    }

    #[test]
    fn autoloop_prompt_errors_when_configured_file_is_missing() {
        let workspace = tempfile::tempdir().unwrap();
        let event_loop = EventLoopConfig {
            prompt: None,
            prompt_file: "missing.md".to_string(),
            ..EventLoopConfig::default()
        };

        let error = resolve_autoloop_prompt(workspace.path(), &event_loop).unwrap_err();

        assert!(error.to_string().contains("reading prompt file"));
        assert!(error.to_string().contains("configured workspace"));
        assert!(!error.to_string().contains("missing.md"));
    }

    #[test]
    fn autoloop_prompt_uses_file_when_inline_text_is_whitespace() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("prompt.md"), "file prompt").unwrap();
        let event_loop = EventLoopConfig {
            prompt: Some("  \n".to_string()),
            prompt_file: "prompt.md".to_string(),
            ..EventLoopConfig::default()
        };

        let prompt = resolve_autoloop_prompt(workspace.path(), &event_loop).unwrap();

        assert_eq!(prompt, "file prompt");
    }

    #[test]
    fn failed_run_ignores_untrusted_error_log_detail() {
        let events = parse_events(concat!(
            r#"{"type":"iteration.start","iteration":0,"maxIterations":1,"runId":"r1"}"#,
            "\n",
            r#"{"type":"log","level":"error","message":"loop stop reason=error completed_iterations=0 detail=untrusted"}"#,
            "\n",
            r#"{"type":"loop.finish","iterations":0,"stopReason":"error","runId":"r1","costUsd":0}"#,
            "\n",
        ));

        assert_eq!(
            recover_failed_run(&events),
            Some(FailedRunRecovery {
                reason: engine_stop_error("error"),
                iterations: 0,
                cost_usd: 0.0,
            })
        );
    }

    #[test]
    fn unknown_stop_reason_is_replaced_with_a_fixed_safe_category() {
        let secret = "TOKEN_SECRET_/physical/workspace";
        let reason = map_stop_reason(secret);

        assert_eq!(
            reason,
            TerminationReason::EngineError {
                detail: Some("unknown engine stop reason".to_string()),
            }
        );
        assert!(!format!("{reason:?}").contains(secret));
    }

    #[test]
    fn failed_run_without_terminal_event_keeps_malformed_jsonl_fallback() {
        let events = parse_events("{not json}\n{\"type\":\"loop.finish\"");

        assert_eq!(recover_failed_run(&events), None);
    }

    #[test]
    fn failed_run_rejects_a_completion_terminal_event() {
        let events = parse_events(
            r#"{"type":"loop.finish","iterations":2,"stopReason":"completed","runId":"r1","costUsd":0.5}"#,
        );

        assert_eq!(recover_failed_run(&events), None);
    }

    #[test]
    fn successful_summary_requires_exact_owned_paths_and_non_symlink_leaves() {
        let workspace = tempfile::tempdir().unwrap();
        let root = prepare_engine_state_root(workspace.path()).unwrap();
        let mut summary = test_summary();
        summary.journal = engine_journal_path(&root);
        summary.memory = engine_memory_path(&root);

        validate_success_summary(&summary, &root).unwrap();
        summary.journal = PathBuf::from("/OUTSIDE/journal.jsonl");
        assert!(validate_success_summary(&summary, &root).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn successful_summary_rejects_symlink_store_leaf() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().unwrap();
        let external = tempfile::NamedTempFile::new().unwrap();
        let root = prepare_engine_state_root(workspace.path()).unwrap();
        symlink(external.path(), engine_journal_path(&root)).unwrap();
        let mut summary = test_summary();
        summary.journal = engine_journal_path(&root);
        summary.memory = engine_memory_path(&root);

        let error = validate_success_summary(&summary, &root).unwrap_err();
        assert!(error.to_string().contains("unsafe symlink"));
        assert!(
            !error
                .to_string()
                .contains(&external.path().display().to_string())
        );
    }

    #[test]
    fn daemon_dependency_failure_creates_no_owned_state() {
        let workspace = tempfile::tempdir().unwrap();
        let error = prepare_daemon_engine_state(workspace.path(), || {
            Err(anyhow::anyhow!("dependency unavailable"))
        })
        .unwrap_err();

        assert!(error.to_string().contains("dependency unavailable"));
        assert!(!workspace.path().join(".ralph/autoloop").exists());
    }

    #[test]
    fn r12_maps_every_canonical_stop_reason_without_unknown_fallback() {
        for reason in [
            "completed",
            "completion_event",
            "completion_promise",
            "verdict_exit",
        ] {
            assert_eq!(
                map_stop_reason(reason),
                TerminationReason::CompletionPromise
            );
        }
        assert_eq!(
            map_stop_reason("max_iterations"),
            TerminationReason::MaxIterations
        );
        assert_eq!(
            map_stop_reason("max_runtime"),
            TerminationReason::MaxRuntime
        );
        assert_eq!(map_stop_reason("cost_budget"), TerminationReason::MaxCost);
        assert_eq!(map_stop_reason("stalled"), TerminationReason::LoopStale);
        assert_eq!(
            map_stop_reason("interrupted"),
            TerminationReason::Interrupted
        );
        assert_eq!(map_stop_reason("suspended"), TerminationReason::Suspended);
        assert_eq!(
            map_stop_reason("completion_held"),
            TerminationReason::CompletionHeld
        );

        let engine_errors = [
            "error",
            "backend_failed",
            "backend_timeout",
            "verdict_takeover",
            "auth_failed",
            "quota_exhausted",
            "rate_limited",
            "transient_error",
            "review_unknown",
            "premature_quit",
            "verdict_unknown",
            "parallel_wave_timeout",
            "parallel_wave_failed",
            "parallel_wave_invalid",
        ];
        for reason in engine_errors {
            assert_eq!(map_stop_reason(reason), engine_stop_error(reason));
        }

        for reason in KNOWN_STOP_REASONS {
            let unknown_fallback = TerminationReason::EngineError {
                detail: Some(format!("unknown engine stop reason: {reason}")),
            };
            assert_ne!(
                map_stop_reason(reason),
                unknown_fallback,
                "canonical stop reason {reason:?} reached the unknown fallback"
            );
        }
    }

    #[test]
    fn failure_class_stop_reasons_exit_non_zero() {
        for reason in [
            "auth_failed",
            "quota_exhausted",
            "rate_limited",
            "transient_error",
            "review_unknown",
            "premature_quit",
            "verdict_unknown",
            "parallel_wave_timeout",
            "parallel_wave_failed",
            "parallel_wave_invalid",
        ] {
            assert_ne!(
                map_stop_reason(reason).exit_code(),
                0,
                "{reason} must not report success"
            );
        }
    }

    #[test]
    fn unknown_stop_reason_is_a_non_zero_engine_error() {
        let reason = map_stop_reason("something_new");
        assert_eq!(
            reason,
            TerminationReason::EngineError {
                detail: Some("unknown engine stop reason".to_string()),
            }
        );
        assert_ne!(reason.exit_code(), 0);
    }

    #[test]
    fn requested_termination_markers_override_reason_and_are_consumed() {
        let temp = tempfile::tempdir().unwrap();
        let ralph_dir = temp.path().join(".ralph");
        std::fs::create_dir_all(&ralph_dir).unwrap();

        std::fs::write(ralph_dir.join("stop-requested"), "").unwrap();
        assert_eq!(
            take_requested_termination(temp.path()),
            Some(TerminationReason::Stopped)
        );
        assert_eq!(take_requested_termination(temp.path()), None);

        std::fs::write(ralph_dir.join("restart-requested"), "").unwrap();
        assert_eq!(
            take_requested_termination(temp.path()),
            Some(TerminationReason::RestartRequested)
        );
        assert_eq!(take_requested_termination(temp.path()), None);
    }
}
