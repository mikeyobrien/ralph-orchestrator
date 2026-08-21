//! v3 cutover: drive the autoloop runtime as ralph's orchestration engine.
//!
//! When `core.engine = "autoloop"`, `ralph run` spawns `autoloop run` as a
//! subprocess via [`AutoloopRunner`] instead of the in-house event loop,
//! consumes its structured `--events` LoopEvent stream, and maps the terminal
//! result onto ralph's [`TerminationReason`]. This is the thin-layer engine swap
//! at the heart of v3: autoloop owns loop execution; ralph coordinates.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use ralph_adapters::{AutoloopRunSummary, AutoloopRunner, events_run_result, parse_events};
use ralph_core::{LoopContext, RalphConfig, RunStats, TerminationReason};

use crate::completion_coord::coordinate_completion;

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
        "backend_failed" | "backend_timeout" | "verdict_takeover" => {
            TerminationReason::ValidationFailure
        }
        _ => TerminationReason::Stopped,
    }
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
    context: Option<LoopContext>,
    auto_merge_override: Option<bool>,
    loop_id: Option<String>,
    use_colors: bool,
    tui: bool,
) -> Result<TerminationReason> {
    let workspace = config.core.workspace_root.clone();

    // Use an explicit preset if configured; otherwise generate one from ralph's
    // native hats topology so existing ralph configs run on the autoloop engine
    // without a hand-authored preset.
    let preset = match config.core.autoloop_preset.as_deref() {
        Some(p) => {
            let preset = resolve(&workspace, p);
            if !preset.join("autoloops.toml").is_file() {
                bail!(
                    "autoloop preset not found (no autoloops.toml): {}",
                    preset.display()
                );
            }
            preset
        }
        None => {
            let preset = workspace.join(".ralph").join("autoloop-preset");
            crate::autoloop_preset_gen::generate_preset(&config, &preset)
                .context("generating an autoloop preset from the hats topology")?;
            tracing::info!(preset = %preset.display(), "engine=autoloop: generated preset from hats config");
            preset
        }
    };

    // The prompt comes from the canonical normalized field.
    let prompt = {
        let pf = config.event_loop.prompt_file.clone();
        if pf.trim().is_empty() {
            String::new()
        } else {
            let path = resolve(&workspace, &pf);
            std::fs::read_to_string(&path)
                .with_context(|| format!("reading prompt file {}", path.display()))?
        }
    };

    // Structured event sink under .ralph/ — the preferred observability channel.
    let events_path = workspace.join(".ralph").join("autoloop-events.ndjson");
    if let Some(parent) = events_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::remove_file(&events_path);

    tracing::info!(
        preset = %preset.display(),
        "engine=autoloop: driving the autoloop runtime as a subprocess"
    );

    // Backend selection is not yet mapped from ralph's backend names to
    // autoloop's backend kinds (#347), so the subprocess uses autoloop's default
    // backend. Surface that explicitly rather than silently ignoring the config.
    if config.cli.backend != "auto" && !config.cli.backend.is_empty() {
        tracing::warn!(
            backend = %config.cli.backend,
            "engine=autoloop: ralph backend selection is not forwarded to autoloop yet; using autoloop's default backend (see #347)"
        );
    }

    let runner = AutoloopRunner::new(preset, prompt.clone(), workspace.clone())
        .events_path(events_path.clone());

    let mut current_events_guard = None;
    let robot_service = if !tui {
        if let Some(loop_context) = context.as_ref()
            && config.robot.enabled
            && loop_context.is_primary()
        {
            current_events_guard = Some(
                crate::autoloop_robot::CurrentEventsGuard::install(&workspace, &events_path)
                    .context("installing Autoloop current-events marker")?,
            );
            let service = crate::autoloop_robot::create_robot_service(&config, loop_context);
            if service.is_none() {
                current_events_guard = None;
            }
            service
        } else {
            None
        }
    } else {
        None
    };

    let start = Instant::now();
    let summary = if tui {
        // In-process TUI: render the autoloop run live by tailing its --events
        // file, concurrent with the subprocess. Resolves Ctrl+C by killing the
        // child (see run_autoloop_with_tui).
        run_autoloop_with_tui(runner, events_path.clone(), workspace.clone())
            .await
            .context("autoloop TUI run failed")?
    } else if let Some(service) = robot_service {
        run_autoloop_with_robot(runner, events_path.clone(), service)
            .await
            .context("autoloop RObot run failed")?
    } else {
        // Headless: AutoloopRunner::run blocks on the subprocess; keep the async
        // runtime free. Unchanged from the pre-TUI path.
        tokio::task::spawn_blocking(move || runner.run())
            .await
            .context("autoloop run task panicked")?
            .context("autoloop run failed")?
    };
    drop(current_events_guard);

    if let Ok(content) = std::fs::read_to_string(&events_path) {
        if let Some(result) = events_run_result(&parse_events(&content)) {
            println!(
                "autoloop engine: run_id={} iterations={} stop_reason={}",
                result.run_id, result.iterations, result.stop_reason
            );
        }
    }

    let reason = take_requested_termination(&workspace)
        .unwrap_or_else(|| map_stop_reason(&summary.stop_reason));

    // Mirror the in-house engine's completion bookkeeping so parallel-loop
    // coordination (merge queue, registry, landing) works under the autoloop
    // engine. autoloop owns iteration/timing; we surface what the summary gives.
    let state = RunStats {
        iterations: summary.iterations,
        elapsed: start.elapsed(),
        cost_usd: summary.cost_usd,
    };

    let auto_merge = auto_merge_override.unwrap_or(config.features.auto_merge);
    let loop_id = loop_id
        .or_else(|| {
            context
                .as_ref()
                .and_then(|c| c.loop_id().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| "primary".to_string());

    coordinate_completion(
        &reason,
        &state,
        context.as_ref(),
        &config.core.scratchpad.path,
        &prompt,
        auto_merge,
        &loop_id,
        use_colors,
    );

    Ok(reason)
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
/// file-backed control protocol.
async fn run_autoloop_with_robot(
    runner: AutoloopRunner,
    events_path: PathBuf,
    service: Box<dyn ralph_proto::RobotService>,
) -> Result<AutoloopRunSummary> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    // Isolate the subprocess tree so cancelling the daemon future can stop both
    // Autoloop and its backend rather than orphaning either process.
    let runner = runner.own_process_group(true);
    let child = runner.spawn().context("spawning the autoloop subprocess")?;
    let done = Arc::new(AtomicBool::new(false));
    let robot_shutdown = service.shutdown_flag();
    let mut process_guard = AutoloopProcessGuard::new(
        child.id(),
        Arc::clone(&done),
        Arc::clone(&robot_shutdown),
    );

    let wait_runner = runner.clone();
    let mut wait_handle = tokio::task::spawn_blocking(move || wait_runner.wait_with_summary(child));

    let bridge_runner = runner.clone();
    let bridge_done = Arc::clone(&done);
    let mut bridge_handle = tokio::task::spawn_blocking(move || {
        crate::autoloop_robot::run_bridge(service, bridge_runner, events_path, bridge_done)
    });

    tokio::select! {
        summary = &mut wait_handle => {
            process_guard.disarm();
            done.store(true, Ordering::Release);
            robot_shutdown.store(true, Ordering::Release);
            let bridge_result = bridge_handle
                .await
                .context("Autoloop RObot bridge panicked")?;
            let summary = summary
                .context("autoloop wait task panicked")?
                .context("autoloop run failed")?;
            bridge_result?;
            Ok(summary)
        }
        bridge = &mut bridge_handle => {
            process_guard.terminate();
            done.store(true, Ordering::Release);
            robot_shutdown.store(true, Ordering::Release);
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
) -> Result<ralph_adapters::AutoloopRunSummary> {
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
        .with_export_workspace_root(workspace);
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
            ralph_tui::run_autoloop_event_reader(events_path, reader_state, cancel_rx).await;
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
    if !completed.load(Ordering::SeqCst) {
        kill_autoloop_group(child_pid);
    }
    // Ensure the reader does its final drain even if the TUI exited first (q).
    let _ = terminated_tx.send(true);

    // Collect the subprocess result, then tear down the auxiliary tasks.
    let summary = wait_handle.await.context("autoloop wait join failed")?;
    let _ = reader_handle.await;
    tui_result.context("TUI render loop failed")?;

    summary
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
        // SIGKILL to an already-dead group is ESRCH and harmless.
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

/// Start a headless orchestration loop for the Telegram bot daemon.
///
/// This is the daemon's [`ralph_proto::StartLoopFn`] target: it loads config,
/// applies the supplied prompt, forces autonomous/headless mode, acquires the
/// primary loop lock, and drives the autoloop engine.
///
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
                tracing::info!("Auto-detected backend: {}", backend);
                config.cli.backend = backend;
            }
            Err(e) => return Err(anyhow::Error::new(e)),
        }
    }

    // Ensure scratchpad directory exists.
    crate::ensure_scratchpad_directory(&config)?;

    // Acquire the loop lock (primary loop).
    let prompt_summary = config.event_loop.prompt.as_deref().unwrap_or("[daemon]");
    let prompt_summary = ralph_core::truncate_with_ellipsis(prompt_summary, 100);

    let _lock_guard = ralph_core::LoopLock::try_acquire(&workspace_root, &prompt_summary)
        .context("Failed to acquire loop lock — another loop may be running")?;

    let loop_context = ralph_core::LoopContext::primary(workspace_root);

    // Drive the loop headlessly via the autoloop engine (daemon: never a TUI).
    // A Telegram `/restart` interrupts Autoloop through its control channel and
    // reruns the same daemon prompt without terminating the daemon process.
    loop {
        let reason = run_autoloop_engine(
            config.clone(),
            Some(loop_context.clone()),
            None,
            None,
            false,
            false,
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

    #[test]
    fn maps_autoloop_stop_reasons_to_termination() {
        assert!(matches!(
            map_stop_reason("completed"),
            TerminationReason::CompletionPromise
        ));
        assert!(matches!(
            map_stop_reason("completion_event"),
            TerminationReason::CompletionPromise
        ));
        assert!(matches!(
            map_stop_reason("completion_promise"),
            TerminationReason::CompletionPromise
        ));
        assert!(matches!(
            map_stop_reason("max_iterations"),
            TerminationReason::MaxIterations
        ));
        assert!(matches!(
            map_stop_reason("cost_budget"),
            TerminationReason::MaxCost
        ));
        assert!(matches!(
            map_stop_reason("interrupted"),
            TerminationReason::Interrupted
        ));
        assert!(matches!(
            map_stop_reason("backend_failed"),
            TerminationReason::ValidationFailure
        ));
        // Unknown reasons fall back to a generic stop.
        assert!(matches!(
            map_stop_reason("something_new"),
            TerminationReason::Stopped
        ));
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
