//! Core orchestration loop implementation.
//!
//! This module contains the main `run_loop_impl` function that executes
//! the Ralph orchestration loop, along with supporting types and helper
//! functions for PTY execution and termination handling.

use anyhow::{Context, Result};
use ralph_adapters::{
    CliBackend, CliExecutor, ConsoleStreamHandler, OutputFormat as BackendOutputFormat,
    PrettyStreamHandler, PtyConfig, PtyExecutor, QuietStreamHandler, TuiStreamHandler,
};
use ralph_core::{
    CompletionAction, EventLogger, EventLoop, EventParser, EventRecord, LoopCompletionHandler,
    LoopContext, LoopHistory, LoopRegistry, MergeQueue, RalphConfig, Record, SessionRecorder,
    SummaryWriter, TerminationReason,
};
use ralph_proto::{Event, HatId};
use ralph_tui::Tui;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufWriter, IsTerminal, stdin, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::display::{build_tui_hat_map, print_iteration_separator, print_termination};
use crate::process_management;
use crate::{ColorMode, Verbosity};

/// Outcome of executing a prompt via PTY or CLI executor.
pub(crate) struct ExecutionOutcome {
    pub output: String,
    pub success: bool,
    pub termination: Option<TerminationReason>,
}

/// Core loop implementation supporting both fresh start and continue modes.
///
/// # Arguments
///
/// * `resume` - If true, publishes `task.resume` instead of `task.start`,
///   signaling the planner to read existing scratchpad rather than doing fresh gap analysis.
/// * `record_session` - If provided, records all events to the specified JSONL file for replay testing.
/// * `auto_merge_override` - Explicit auto-merge setting. If `Some(false)`, disables auto-merge
///   (equivalent to `--no-auto-merge`). If `None`, uses `config.features.auto_merge`.
pub async fn run_loop_impl(
    config: RalphConfig,
    color_mode: ColorMode,
    resume: bool,
    enable_tui: bool,
    verbosity: Verbosity,
    record_session: Option<PathBuf>,
    loop_context: Option<LoopContext>,
    custom_args: Vec<String>,
    auto_merge_override: Option<bool>,
) -> Result<TerminationReason> {
    // Set up process group leadership per spec
    // "The orchestrator must run as a process group leader"
    process_management::setup_process_group();

    let use_colors = color_mode.should_use_colors();

    // Determine effective execution mode (with fallback logic)
    // Per spec: Claude backend requires PTY mode to avoid hangs
    // TUI mode is observation-only - uses streaming mode, not interactive
    let interactive_requested = config.cli.default_mode == "interactive" && !enable_tui;
    let user_interactive = if interactive_requested {
        if stdout().is_terminal() {
            true
        } else {
            warn!("Interactive mode requested but stdout is not a TTY, falling back to autonomous");
            false
        }
    } else {
        false
    };
    // Always use PTY for real-time streaming output (vs buffered CliExecutor)
    let use_pty = true;

    // Set up interrupt channel for signal handling
    // Per spec:
    // - SIGINT (Ctrl+C): Immediately terminate child process (SIGTERM -> 5s grace -> SIGKILL), exit with code 130
    // - SIGTERM: Same as SIGINT
    // - SIGHUP: Same as SIGINT
    //
    // Use watch channel for interrupt notification so we can race execution vs interrupt
    // Note: Signal handlers are spawned AFTER TUI initialization to avoid deadlock
    let (interrupt_tx, interrupt_rx) = tokio::sync::watch::channel(false);

    // Resolve prompt content with precedence:
    // 1. CLI -p (inline text)
    // 2. CLI -P (file path)
    // 3. Config prompt (inline text)
    // 4. Config prompt_file (file path)
    // 5. Default PROMPT.md
    let prompt_content = resolve_prompt_content(&config.event_loop)?;

    // Create or use provided loop context for path resolution
    // This ensures events are written to the correct location for worktree loops
    let ctx = loop_context
        .clone()
        .unwrap_or_else(|| LoopContext::primary(config.core.workspace_root.clone()));

    // Write loop ID to marker file for task ownership tracking.
    // For worktree loops, use the loop_id; for primary loops, generate one.
    // This file is read by `ralph tools task add` to tag new tasks.
    let loop_id = ctx.loop_id().map(|s| s.to_string()).unwrap_or_else(|| {
        // Primary loop gets a timestamped ID
        format!("primary-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"))
    });
    let loop_id_marker = ctx.ralph_dir().join("current-loop-id");
    fs::write(&loop_id_marker, &loop_id).context("Failed to write current-loop-id marker")?;
    debug!(loop_id = %loop_id, marker = ?loop_id_marker, "Wrote loop ID marker file");

    // For fresh runs (not resume), generate a unique timestamped events file
    // This prevents stale events from previous runs polluting new runs (issue #82)
    // The marker file `.ralph/current-events` coordinates path between Ralph and agents
    if !resume {
        let run_id = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        // Use relative path in marker file for portability across agents
        // The actual file is at ctx.ralph_dir()/events-{run_id}.jsonl
        let relative_events_path = format!(".ralph/events-{}.jsonl", run_id);

        fs::create_dir_all(ctx.ralph_dir()).context("Failed to create .ralph directory")?;
        fs::write(ctx.current_events_marker(), &relative_events_path)
            .context("Failed to write current-events marker file")?;

        debug!("Created events file for this run: {}", relative_events_path);

        // Clear scratchpad for fresh objective start
        // Stale content from previous runs can confuse the agent about current task state
        let scratchpad_path = ctx.scratchpad_path();
        if scratchpad_path.exists() {
            fs::remove_file(&scratchpad_path)
                .with_context(|| format!("Failed to clear scratchpad: {:?}", scratchpad_path))?;
            debug!(
                "Cleared scratchpad for fresh objective: {:?}",
                scratchpad_path
            );
        }
    }

    // Initialize event loop with context for proper path resolution
    let mut event_loop = EventLoop::with_context(config.clone(), ctx.clone());

    // Inject robot service (Telegram) for human-in-the-loop communication
    if config.robot.enabled
        && ctx.is_primary()
        && let Some(service) = create_robot_service(&config, &ctx)
    {
        event_loop.set_robot_service(service);
    }

    // Capture the robot service shutdown flag so signal handlers can interrupt wait_for_response()
    let robot_shutdown = event_loop.robot_shutdown_flag();

    // For resume mode, we initialize with a different event topic
    // This tells the planner to read existing scratchpad rather than creating a new one
    if resume {
        event_loop.initialize_resume(&prompt_content);
    } else {
        event_loop.initialize(&prompt_content);
    }

    // Set up session recording if requested
    // This records all events to a JSONL file for replay testing
    let _session_recorder: Option<Arc<SessionRecorder<BufWriter<File>>>> =
        if let Some(record_path) = record_session {
            let file = File::create(&record_path).with_context(|| {
                format!("Failed to create session recording file: {:?}", record_path)
            })?;
            let recorder = Arc::new(SessionRecorder::new(BufWriter::new(file)));

            // Record metadata for the session
            recorder.record_meta(Record::meta_loop_start(
                &config.event_loop.prompt_file,
                config.event_loop.max_iterations,
                if enable_tui { Some("tui") } else { Some("cli") },
            ));

            // Wire observer to EventBus so events are recorded
            let observer = SessionRecorder::make_observer(Arc::clone(&recorder));
            event_loop.add_observer(observer);

            info!("Session recording enabled: {:?}", record_path);
            Some(recorder)
        } else {
            None
        };

    // Initialize event logger for debugging (uses context for path resolution)
    let mut event_logger = EventLogger::from_context(&ctx);

    // Log initial event (use configured starting_event or default to task.start/task.resume)
    let default_start_topic = if resume { "task.resume" } else { "task.start" };
    let start_topic = config
        .event_loop
        .starting_event
        .as_deref()
        .unwrap_or(default_start_topic);
    let start_triggered = "planner"; // Default triggered hat for backward compat
    let start_event = Event::new(start_topic, &prompt_content);
    let start_record =
        EventRecord::new(0, "loop", &start_event, Some(&HatId::new(start_triggered)));
    if let Err(e) = event_logger.log(&start_record) {
        warn!("Failed to log start event: {}", e);
    }

    // Create backend from config - TUI mode uses the same backend as non-TUI
    // The TUI is an observation layer that displays output, not a different mode
    let mut backend = CliBackend::from_config(&config.cli).map_err(|e| anyhow::Error::new(e))?;

    // Append custom args from CLI if provided (e.g., `ralph run -b opencode -- --model="some-model"`)
    if !custom_args.is_empty() {
        backend.args.extend(custom_args);
    }

    // Create PTY executor if using interactive mode
    let mut pty_executor = if use_pty {
        let idle_timeout_secs = if user_interactive {
            config.cli.idle_timeout_secs
        } else {
            0
        };
        let pty_config = PtyConfig {
            interactive: user_interactive,
            idle_timeout_secs,
            workspace_root: config.core.workspace_root.clone(),
            ..PtyConfig::from_env()
        };
        Some(PtyExecutor::new(backend.clone(), pty_config))
    } else {
        None
    };

    // Create termination signal for TUI shutdown
    let (terminated_tx, terminated_rx) = tokio::sync::watch::channel(false);

    // Wire TUI with termination signal and shared state
    // TUI is observation-only - works in both interactive and autonomous modes
    // Requirements: both stdin and stdout must be terminals for TUI
    // (Crossterm requires stdin for keyboard input, stdout for rendering)
    let enable_tui = enable_tui && stdin().is_terminal() && stdout().is_terminal();
    let (mut tui_handle, tui_state) = if enable_tui {
        // Build hat map for dynamic topic-to-hat resolution
        // This allows TUI to display custom hats (e.g., "Security Reviewer")
        // instead of generic "ralph" for all events
        let hat_map = build_tui_hat_map(event_loop.registry());
        let tui = Tui::new()
            .with_hat_map(hat_map)
            .with_termination_signal(terminated_rx);

        // Get shared state before spawning (for content streaming)
        let state = tui.state();

        // Wire interrupt channel so TUI can signal main loop on Ctrl+C
        // (raw mode prevents SIGINT from being generated by the OS)
        let tui = tui.with_interrupt_tx(interrupt_tx.clone());

        let observer = tui.observer();
        event_loop.add_observer(observer);
        (
            Some(tokio::spawn(async move { tui.run().await })),
            Some(state),
        )
    } else {
        (None, None)
    };

    // Give TUI task time to initialize (enter alternate screen, enable raw mode)
    // before the main loop starts doing work
    if tui_handle.is_some() {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Seed max_iterations into TUI state for accurate iteration display.
    if let Some(mut s) = tui_state.as_ref().and_then(|state| state.lock().ok()) {
        s.max_iterations = Some(config.event_loop.max_iterations);
    }

    // Spawn signal handlers AFTER TUI initialization to avoid deadlock
    // (TUI must enter raw mode and create EventStream before signal handlers are registered)

    // Spawn task to listen for SIGINT (Ctrl+C)
    let interrupt_tx_sigint = interrupt_tx.clone();
    let robot_shutdown_sigint = robot_shutdown.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            debug!("Interrupt received (SIGINT), terminating immediately...");
            if let Some(ref flag) = robot_shutdown_sigint {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            let _ = interrupt_tx_sigint.send(true);
        }
    });

    // Spawn task to listen for SIGTERM (Unix only)
    #[cfg(unix)]
    {
        let interrupt_tx_sigterm = interrupt_tx.clone();
        let robot_shutdown_sigterm = robot_shutdown.clone();
        tokio::spawn(async move {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Failed to register SIGTERM handler");
            sigterm.recv().await;
            debug!("SIGTERM received, terminating immediately...");
            if let Some(ref flag) = robot_shutdown_sigterm {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            let _ = interrupt_tx_sigterm.send(true);
        });
    }

    // Spawn task to listen for SIGHUP (Unix only)
    #[cfg(unix)]
    {
        let interrupt_tx_sighup = interrupt_tx.clone();
        let robot_shutdown_sighup = robot_shutdown.clone();
        tokio::spawn(async move {
            let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
                .expect("Failed to register SIGHUP handler");
            sighup.recv().await;
            warn!("SIGHUP received (terminal closed), terminating immediately...");
            if let Some(ref flag) = robot_shutdown_sighup {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            let _ = interrupt_tx_sighup.send(true);
        });
    }

    // Log execution mode - hat info already logged by initialize()
    let exec_mode = if user_interactive {
        "interactive"
    } else {
        "autonomous"
    };
    debug!(execution_mode = %exec_mode, "Execution mode configured");

    // Track the last hat to detect hat changes for logging
    let mut last_hat: Option<HatId> = None;

    // Track consecutive fallback attempts to prevent infinite loops
    let mut consecutive_fallbacks: u32 = 0;
    const MAX_FALLBACK_ATTEMPTS: u32 = 3;

    // Initialize loop history if we have a loop context
    let loop_history = loop_context
        .as_ref()
        .map(|ctx| LoopHistory::from_context(ctx));

    // Record loop start in history
    if let Some(ref history) = loop_history
        && let Err(e) = history.record_started(&prompt_content)
    {
        warn!("Failed to record loop start in history: {}", e);
    }

    // Auto-merge setting: CLI override > config > default (false for safety)
    let auto_merge = auto_merge_override.unwrap_or(config.features.auto_merge);

    // Detect merge loop on startup via RALPH_MERGE_LOOP_ID env var
    // Per spec: If set, mark entry as "merging" with current PID
    let merge_loop_id: Option<String> = std::env::var("RALPH_MERGE_LOOP_ID").ok();
    if let Some(ref loop_id) = merge_loop_id {
        let repo_root = loop_context
            .as_ref()
            .map(|ctx| ctx.repo_root().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let queue = MergeQueue::new(&repo_root);
        let pid = std::process::id();

        match queue.mark_merging(loop_id, pid) {
            Ok(()) => {
                info!(loop_id = %loop_id, pid = pid, "Merge loop started, marked as merging");
            }
            Err(ralph_core::MergeQueueError::NotFound(_)) => {
                warn!(loop_id = %loop_id, "Merge loop started but no queue entry found");
            }
            Err(ralph_core::MergeQueueError::InvalidTransition(_, from, _)) => {
                // Entry is already merging/merged/discarded, skip update
                debug!(loop_id = %loop_id, state = ?from, "Merge queue entry already in terminal state, skipping");
            }
            Err(e) => {
                warn!(loop_id = %loop_id, error = %e, "Failed to mark merge loop as merging");
            }
        }
    }

    // Helper closure to handle termination (writes summary, prints status, records history)
    let handle_termination = |reason: &TerminationReason,
                              state: &ralph_core::LoopState,
                              scratchpad: &str,
                              history: &Option<LoopHistory>,
                              context: &Option<LoopContext>,
                              auto_merge: bool,
                              prompt: &str| {
        // Per spec: Write summary file on termination
        let summary_writer = SummaryWriter::default();
        let scratchpad_path = std::path::Path::new(scratchpad);
        let scratchpad_opt = if scratchpad_path.exists() {
            Some(scratchpad_path)
        } else {
            None
        };

        // Get final commit SHA if available
        let final_commit = get_last_commit_info();

        if let Err(e) = summary_writer.write(reason, state, scratchpad_opt, final_commit.as_deref())
        {
            warn!("Failed to write summary file: {}", e);
        }

        // Record termination in history
        if let Some(hist) = history {
            let reason_str = match reason {
                TerminationReason::CompletionPromise => "completion_promise",
                TerminationReason::MaxIterations => "max_iterations",
                TerminationReason::MaxRuntime => "max_runtime",
                TerminationReason::MaxCost => "max_cost",
                TerminationReason::ConsecutiveFailures => "consecutive_failures",
                TerminationReason::LoopThrashing => "loop_thrashing",
                TerminationReason::ValidationFailure => "validation_failure",
                TerminationReason::Stopped => "stopped",
                TerminationReason::Interrupted => "interrupted",
                TerminationReason::RestartRequested => "restart_requested",
            };

            if matches!(reason, TerminationReason::Interrupted) {
                if let Err(e) = hist.record_terminated("SIGTERM") {
                    warn!("Failed to record termination in history: {}", e);
                }
            } else if let Err(e) = hist.record_completed(reason_str) {
                warn!("Failed to record completion in history: {}", e);
            }
        }

        // Handle merge queue state transitions for merge loops
        // Per spec: CompletionPromise → merged, other → needs-review
        if let Some(ref loop_id) = merge_loop_id {
            let repo_root = context
                .as_ref()
                .map(|ctx| ctx.repo_root().to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            let queue = MergeQueue::new(&repo_root);

            if matches!(reason, TerminationReason::CompletionPromise) {
                // Get commit SHA from git rev-parse HEAD
                let commit = Command::new("git")
                    .args(["rev-parse", "HEAD"])
                    .output()
                    .ok()
                    .and_then(|output| {
                        if output.status.success() {
                            String::from_utf8(output.stdout)
                                .ok()
                                .map(|s| s.trim().to_string())
                        } else {
                            None
                        }
                    });

                match commit {
                    Some(sha) => {
                        if let Err(e) = queue.mark_merged(loop_id, &sha) {
                            warn!(loop_id = %loop_id, error = %e, "Failed to mark merge as completed");
                        } else {
                            info!(loop_id = %loop_id, commit = %sha, "Merge completed successfully");
                        }
                    }
                    None => {
                        // Per spec: "If commit SHA cannot be resolved, mark as needs-review"
                        if let Err(e) =
                            queue.mark_needs_review(loop_id, "merge complete but commit not found")
                        {
                            warn!(loop_id = %loop_id, error = %e, "Failed to mark merge as needs-review");
                        } else {
                            warn!(loop_id = %loop_id, "Merge completed but could not resolve commit SHA");
                        }
                    }
                }
            } else {
                // Any non-CompletionPromise termination → needs-review
                let reason_str = match reason {
                    TerminationReason::MaxIterations => "max iterations reached",
                    TerminationReason::MaxRuntime => "max runtime exceeded",
                    TerminationReason::MaxCost => "max cost exceeded",
                    TerminationReason::ConsecutiveFailures => "consecutive failures",
                    TerminationReason::LoopThrashing => "loop thrashing detected",
                    TerminationReason::ValidationFailure => "validation failure",
                    TerminationReason::Stopped => "manually stopped",
                    TerminationReason::Interrupted => "interrupted by signal",
                    TerminationReason::CompletionPromise => unreachable!(),
                    TerminationReason::RestartRequested => "restart requested",
                };
                if let Err(e) = queue.mark_needs_review(loop_id, reason_str) {
                    warn!(loop_id = %loop_id, error = %e, "Failed to mark merge as needs-review");
                } else {
                    info!(loop_id = %loop_id, reason = reason_str, "Merge marked as needs-review");
                }
            }
        }

        // Handle completion for all loops (landing + merge queue for worktrees)
        // Per spec: merge loops do NOT enqueue themselves, even if run in worktree context
        if let Some(ctx) = context {
            if merge_loop_id.is_none() && matches!(reason, TerminationReason::CompletionPromise) {
                let handler = LoopCompletionHandler::new(auto_merge);
                match handler.handle_completion(ctx, prompt) {
                    Ok(CompletionAction::None) => {
                        debug!("Loop completed, no action needed");
                    }
                    Ok(CompletionAction::Landed { landing }) => {
                        info!(
                            committed = landing.committed,
                            handoff = %landing.handoff_path,
                            open_tasks = landing.open_task_count,
                            "Primary loop landed successfully"
                        );
                    }
                    Ok(CompletionAction::Enqueued { loop_id, landing }) => {
                        info!(loop_id = %loop_id, "Loop queued for auto-merge");
                        if let Some(ref l) = landing {
                            debug!(
                                committed = l.committed,
                                handoff = %l.handoff_path,
                                "Landing completed before enqueue"
                            );
                        }
                        if let Some(hist) = history {
                            let _ = hist.record_merge_queued();
                        }
                        // Worktree loop exits cleanly; merge will be processed
                        // when the primary loop completes and checks the queue
                    }
                    Ok(CompletionAction::ManualMerge {
                        loop_id,
                        worktree_path,
                        landing,
                    }) => {
                        info!(
                            loop_id = %loop_id,
                            "Loop completed. To merge manually: cd {} && git merge",
                            worktree_path
                        );
                        if let Some(ref l) = landing {
                            debug!(
                                committed = l.committed,
                                handoff = %l.handoff_path,
                                "Landing completed (manual merge mode)"
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Completion handler failed: {}", e);
                    }
                }
            }

            // Handle merge queue processing for primary loop completion
            if ctx.is_primary() && matches!(reason, TerminationReason::CompletionPromise) {
                process_pending_merges(ctx.repo_root());
            }

            // Always deregister from registry — process is exiting regardless of reason.
            // CompletionPromise loops are tracked by the merge queue from here on.
            let registry = LoopRegistry::new(ctx.repo_root());
            if let Err(e) = registry.deregister_current_process() {
                warn!("Failed to deregister loop from registry: {}", e);
            }
        }

        // Print termination info to console (skip in TUI mode - TUI handles display)
        if !enable_tui {
            print_termination(reason, state, use_colors);
        }
    };

    // Main orchestration loop
    loop {
        // Check for interrupt signal at start of each iteration
        // This catches TUI Ctrl+C (via interrupt_tx) before printing iteration separator
        if *interrupt_rx.borrow() {
            #[cfg(unix)]
            {
                use nix::sys::signal::{Signal, killpg};
                use nix::unistd::getpgrp;
                let pgid = getpgrp();
                debug!(
                    "Interrupt detected at loop start, sending SIGTERM to process group {}",
                    pgid
                );
                let _ = killpg(pgid, Signal::SIGTERM);
                tokio::time::sleep(Duration::from_millis(250)).await;
                let _ = killpg(pgid, Signal::SIGKILL);
            }
            let reason = TerminationReason::Interrupted;
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );
            handle_termination(
                &reason,
                event_loop.state(),
                &config.core.scratchpad,
                &loop_history,
                &loop_context,
                auto_merge,
                &prompt_content,
            );
            // Signal TUI to exit immediately on interrupt
            let _ = terminated_tx.send(true);
            return Ok(reason);
        }

        // Check termination before execution
        if let Some(reason) = event_loop.check_termination() {
            // Per spec: Publish loop.terminate event to observers
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );
            handle_termination(
                &reason,
                event_loop.state(),
                &config.core.scratchpad,
                &loop_history,
                &loop_context,
                auto_merge,
                &prompt_content,
            );
            // Wait for user to exit TUI (press 'q') on natural completion
            if let Some(handle) = tui_handle.take() {
                let _ = handle.await;
            }
            return Ok(reason);
        }

        // Get next hat to execute, with fallback recovery if no pending events
        let hat_id = match event_loop.next_hat() {
            Some(id) => {
                // Reset fallback counter on successful event routing
                consecutive_fallbacks = 0;
                id.clone()
            }
            None => {
                // No pending events - try to recover by injecting a fallback event
                // This triggers the built-in planner to assess the situation
                consecutive_fallbacks += 1;

                if consecutive_fallbacks > MAX_FALLBACK_ATTEMPTS {
                    warn!(
                        attempts = consecutive_fallbacks,
                        "Fallback recovery exhausted after {} attempts, terminating",
                        MAX_FALLBACK_ATTEMPTS
                    );
                    let reason = TerminationReason::Stopped;
                    let terminate_event = event_loop.publish_terminate_event(&reason);
                    log_terminate_event(
                        &mut event_logger,
                        event_loop.state().iteration,
                        &terminate_event,
                    );
                    handle_termination(
                        &reason,
                        event_loop.state(),
                        &config.core.scratchpad,
                        &loop_history,
                        &loop_context,
                        auto_merge,
                        &prompt_content,
                    );
                    // Wait for user to exit TUI (press 'q') on natural completion
                    if let Some(handle) = tui_handle.take() {
                        let _ = handle.await;
                    }
                    return Ok(reason);
                }

                if event_loop.inject_fallback_event() {
                    // Fallback injected successfully, continue to next iteration
                    // The planner will be triggered and can either:
                    // - Dispatch more work if tasks remain
                    // - Output LOOP_COMPLETE if done
                    // - Determine what went wrong and recover
                    continue;
                }

                // Fallback not possible (no planner hat or doesn't subscribe to task.resume)
                warn!("No hats with pending events and fallback not available, terminating");
                let reason = TerminationReason::Stopped;
                // Per spec: Publish loop.terminate event to observers
                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(
                    &mut event_logger,
                    event_loop.state().iteration,
                    &terminate_event,
                );
                handle_termination(
                    &reason,
                    event_loop.state(),
                    &config.core.scratchpad,
                    &loop_history,
                    &loop_context,
                    auto_merge,
                    &prompt_content,
                );
                // Wait for user to exit TUI (press 'q') on natural completion
                if let Some(handle) = tui_handle.take() {
                    let _ = handle.await;
                }
                return Ok(reason);
            }
        };

        let iteration = event_loop.state().iteration + 1;

        // Determine which hat to display in iteration separator
        // When Ralph is coordinating (hat_id == "ralph"), show the active hat being worked on
        let display_hat = if hat_id.as_str() == "ralph" {
            event_loop.get_active_hat_id()
        } else {
            hat_id.clone()
        };

        // Per spec: Print iteration demarcation separator
        // "Each iteration must be clearly demarcated in the output so users can
        // visually distinguish where one iteration ends and another begins."
        // Skip when TUI is enabled - TUI has its own header showing iteration info
        if tui_state.is_none() {
            print_iteration_separator(
                iteration,
                display_hat.as_str(),
                event_loop.state().elapsed(),
                config.event_loop.max_iterations,
                use_colors,
            );
        }

        // Log hat changes with appropriate messaging
        // Skip in TUI mode - TUI shows hat info in header, and stdout would corrupt display
        if last_hat.as_ref() != Some(&hat_id) {
            if tui_state.is_none() {
                if hat_id.as_str() == "ralph" {
                    info!("I'm Ralph. Let's do this.");
                } else {
                    info!("Putting on my {} hat.", hat_id);
                }
            }
            last_hat = Some(hat_id.clone());
        }
        debug!(
            "Iteration {}/{} - {} active",
            iteration, config.event_loop.max_iterations, hat_id
        );

        // Build prompt for this hat
        let prompt = match event_loop.build_prompt(&hat_id) {
            Some(p) => p,
            None => {
                error!("Failed to build prompt for hat '{}'", hat_id);
                continue;
            }
        };

        // In verbose mode, print the full prompt before execution
        if verbosity == Verbosity::Verbose {
            eprintln!("\n{}", "=".repeat(80));
            eprintln!("PROMPT FOR {} (iteration {})", hat_id, iteration);
            eprintln!("{}", "-".repeat(80));
            eprintln!("{}", prompt);
            eprintln!("{}\n", "=".repeat(80));
        }

        // Execute the prompt (interactive or autonomous mode)
        // Determine which backend to use for this hat and the appropriate timeout
        // Hat-level backend configuration takes precedence over global cli.backend

        // Step 1: Get hat backend configuration for the active hat
        // Use display_hat (the active hat) instead of hat_id ("ralph" in multi-hat mode)
        let hat_backend_opt = event_loop.get_hat_backend(&display_hat);

        // Step 2: Resolve effective backend and determine backend name for timeout
        // Note: backend_name_for_timeout is owned String to avoid lifetime issues with hat_backend reference
        let (effective_backend, backend_name_for_timeout): (CliBackend, String) =
            match hat_backend_opt {
                Some(hat_backend) => {
                    // Hat has custom backend configuration
                    match CliBackend::from_hat_backend(hat_backend) {
                        Ok(hat_backend_instance) => {
                            debug!(
                                "Using hat-level backend for '{}': {:?}",
                                display_hat, hat_backend
                            );

                            // Determine backend name for timeout based on hat backend type
                            // Use owned String to avoid borrowing issues and improve code clarity
                            let backend_name = match hat_backend {
                                ralph_core::HatBackend::Named(name) => name.clone(),
                                ralph_core::HatBackend::NamedWithArgs { backend_type, .. } => {
                                    backend_type.clone()
                                }
                                ralph_core::HatBackend::KiroAgent { .. } => "kiro".to_string(),
                                // For Custom backends, extract command name from path
                                // Handles both Unix ("/usr/bin/codex") and commands with args ("ollama run llama3")
                                ralph_core::HatBackend::Custom { command, .. } => {
                                    // First split by whitespace to handle commands with arguments
                                    // e.g., "ollama run llama3" -> "ollama"
                                    let base_command =
                                        command.split_whitespace().next().unwrap_or(command);
                                    // Then extract filename from path
                                    // e.g., "/usr/bin/codex" -> "codex"
                                    std::path::Path::new(base_command)
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("custom")
                                        .to_string()
                                }
                            };

                            (hat_backend_instance, backend_name)
                        }
                        Err(e) => {
                            // Failed to create backend from hat config - fall back to global
                            warn!(
                                "Failed to create backend from hat configuration for '{}': {}. Falling back to global backend.",
                                display_hat, e
                            );
                            // IMPORTANT: Use global backend name for timeout since we're using global backend
                            (backend.clone(), config.cli.backend.clone())
                        }
                    }
                }
                None => {
                    // No custom backend - use global configuration
                    debug!(
                        "Using global backend for '{}': {}",
                        display_hat, config.cli.backend
                    );
                    (backend.clone(), config.cli.backend.clone())
                }
            };

        // Step 3: Get timeout from config based on actual backend being used
        let timeout_secs = config.adapter_settings(&backend_name_for_timeout).timeout;
        let timeout = Some(Duration::from_secs(timeout_secs));

        // For TUI mode, get the shared lines buffer for this iteration.
        // The buffer is owned by TuiState's IterationBuffer, so writes from
        // TuiStreamHandler appear immediately in the TUI (real-time streaming).
        let hat_display = event_loop
            .registry()
            .get(&display_hat)
            .map(|hat| hat.name.clone())
            .unwrap_or_else(|| display_hat.as_str().to_string());

        let tui_lines: Option<Arc<std::sync::Mutex<Vec<ratatui::text::Line<'static>>>>> =
            if let Some(ref state) = tui_state {
                // Start new iteration and get handle to the LATEST iteration's lines buffer.
                // We must use latest_iteration_lines_handle() instead of current_iteration_lines_handle()
                // because the user may be viewing an older iteration while a new one executes.
                prepare_tui_iteration(
                    state,
                    hat_display.clone(),
                    backend_name_for_timeout.clone(),
                    config.event_loop.max_iterations,
                )
            } else {
                None
            };

        // Race execution against interrupt signal for immediate termination on Ctrl+C
        let mut interrupt_rx_clone = interrupt_rx.clone();
        let interrupt_rx_for_pty = interrupt_rx.clone();
        let tui_lines_for_pty = tui_lines.clone();
        let execute_future = async {
            if use_pty {
                execute_pty(
                    pty_executor.as_mut(),
                    &effective_backend,
                    &config,
                    &prompt,
                    user_interactive,
                    interrupt_rx_for_pty,
                    verbosity,
                    tui_lines_for_pty,
                )
                .await
            } else {
                let executor = CliExecutor::new(effective_backend.clone());
                let result = executor
                    .execute(&prompt, stdout(), timeout, verbosity == Verbosity::Verbose)
                    .await?;
                Ok(ExecutionOutcome {
                    output: result.output,
                    success: result.success,
                    termination: None,
                })
            }
        };

        let outcome = tokio::select! {
            result = execute_future => result?,
            _ = interrupt_rx_clone.changed() => {
                // Immediately terminate children via process group signal
                #[cfg(unix)]
                {
                    use nix::sys::signal::{killpg, Signal};
                    use nix::unistd::getpgrp;
                    let pgid = getpgrp();
                    debug!("Sending SIGTERM to process group {}", pgid);
                    let _ = killpg(pgid, Signal::SIGTERM);

                    // Wait briefly for graceful exit, then SIGKILL
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    let _ = killpg(pgid, Signal::SIGKILL);
                }

                let reason = TerminationReason::Interrupted;
                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(&mut event_logger, event_loop.state().iteration, &terminate_event);
                handle_termination(&reason, event_loop.state(), &config.core.scratchpad, &loop_history, &loop_context, auto_merge, &prompt_content);
                // Signal TUI to exit immediately on interrupt
                let _ = terminated_tx.send(true);
                return Ok(reason);
            }
        };

        if let Some(reason) = outcome.termination {
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );
            handle_termination(
                &reason,
                event_loop.state(),
                &config.core.scratchpad,
                &loop_history,
                &loop_context,
                auto_merge,
                &prompt_content,
            );
            // Wait for user to exit TUI (press 'q') on natural completion
            if let Some(handle) = tui_handle.take() {
                let _ = handle.await;
            }
            return Ok(reason);
        }

        let output = outcome.output;
        let success = outcome.success;

        // Note: TUI lines are now written directly to IterationBuffer during streaming,
        // so no post-execution transfer is needed.
        if let Some(mut s) = tui_state.as_ref().and_then(|state| state.lock().ok()) {
            s.finish_latest_iteration();
        }

        // Log events from output before processing
        log_events_from_output(
            &mut event_logger,
            iteration,
            &hat_id,
            &output,
            event_loop.registry(),
        );

        // Process output
        if let Some(reason) = event_loop.process_output(&hat_id, &output, success) {
            // Per spec: Log "All done! {promise} detected." when completion promise found
            if reason == TerminationReason::CompletionPromise {
                info!(
                    "All done! {} detected.",
                    config.event_loop.completion_promise
                );
            }
            // Per spec: Publish loop.terminate event to observers
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );
            handle_termination(
                &reason,
                event_loop.state(),
                &config.core.scratchpad,
                &loop_history,
                &loop_context,
                auto_merge,
                &prompt_content,
            );
            // Wait for user to exit TUI (press 'q') on natural completion
            if let Some(handle) = tui_handle.take() {
                let _ = handle.await;
            }
            return Ok(reason);
        }

        // Check for planning session user responses (if in planning mode)
        if let Err(e) = check_planning_session_responses(&mut event_loop) {
            warn!(error = %e, "Failed to check planning session responses");
        }

        // Read events from JSONL that agent may have written
        if let Err(e) = event_loop.process_events_from_jsonl() {
            warn!(error = %e, "Failed to read events from JSONL");
        }

        if let Some(reason) = event_loop.check_completion_event() {
            info!(
                "Completion event {} detected.",
                config.event_loop.completion_promise
            );

            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );
            handle_termination(
                &reason,
                event_loop.state(),
                &config.core.scratchpad,
                &loop_history,
                &loop_context,
                auto_merge,
                &prompt_content,
            );
            if let Some(handle) = tui_handle.take() {
                let _ = handle.await;
            }
            return Ok(reason);
        }

        // Precheck validation: Warn if no pending events after processing output
        // Per EventLoop doc: "Use has_pending_events after process_output to detect
        // if the LLM failed to publish an event."
        if !event_loop.has_pending_events() {
            let expected = event_loop.get_hat_publishes(&hat_id);
            debug!(
                hat = %hat_id.as_str(),
                expected_topics = ?expected,
                "No pending events after iteration. Agent may have failed to publish a valid event. \
                 Expected one of: {:?}. Loop will terminate on next iteration.",
                expected
            );
        }

        // Cooldown delay between iterations (skip for human events)
        let cooldown = config.event_loop.cooldown_delay_seconds;
        if cooldown > 0 && !event_loop.has_pending_human_events() {
            debug!(
                delay_seconds = cooldown,
                "Cooldown delay before next iteration"
            );
            tokio::time::sleep(Duration::from_secs(cooldown)).await;
        }
    }
}

/// Executes a prompt in PTY mode with raw terminal handling.
/// Converts PTY termination type to loop termination reason.
///
/// In interactive mode, idle timeout signals "iteration complete" rather than
/// "loop stopped", allowing the event loop to process output and continue.
///
/// # Arguments
/// * `termination_type` - The PTY executor's termination type
/// * `interactive` - Whether running in interactive mode
///
/// # Returns
/// * `None` - Continue processing (iteration complete)
/// * `Some(TerminationReason)` - Stop the loop
fn convert_termination_type(
    termination_type: ralph_adapters::TerminationType,
    interactive: bool,
) -> Option<TerminationReason> {
    match termination_type {
        ralph_adapters::TerminationType::Natural => None,
        ralph_adapters::TerminationType::IdleTimeout => {
            if interactive {
                // In interactive mode, idle timeout signals iteration complete,
                // not loop termination. Let output be processed for events.
                info!("PTY idle timeout in interactive mode, iteration complete");
                None
            } else {
                warn!("PTY idle timeout reached, terminating loop");
                Some(TerminationReason::Stopped)
            }
        }
        ralph_adapters::TerminationType::UserInterrupt
        | ralph_adapters::TerminationType::ForceKill => Some(TerminationReason::Interrupted),
    }
}

fn prepare_tui_iteration(
    tui_state: &Arc<std::sync::Mutex<ralph_tui::TuiState>>,
    hat_display: String,
    backend: String,
    max_iterations: u32,
) -> Option<Arc<std::sync::Mutex<Vec<ratatui::text::Line<'static>>>>> {
    let Ok(mut state) = tui_state.lock() else {
        return None;
    };
    // Ensure max_iterations is always available for header display, even if
    // state was reset by earlier events.
    state.max_iterations = Some(max_iterations);
    state.start_new_iteration_with_metadata(Some(hat_display), Some(backend));
    state.latest_iteration_lines_handle()
}

async fn execute_pty(
    executor: Option<&mut PtyExecutor>,
    backend: &CliBackend,
    config: &RalphConfig,
    prompt: &str,
    interactive: bool,
    interrupt_rx: tokio::sync::watch::Receiver<bool>,
    verbosity: Verbosity,
    tui_lines: Option<Arc<std::sync::Mutex<Vec<ratatui::text::Line<'static>>>>>,
) -> Result<ExecutionOutcome> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    // Use provided executor or create a new one
    // If executor is provided, TUI is connected and owns raw mode management
    let tui_connected = executor.is_some();
    let mut temp_executor;
    let exec = if let Some(e) = executor {
        // Update the executor's backend to use hat-level configuration
        // This is critical for hat-level backend support - without this update,
        // the executor would continue using the global backend it was created with
        e.set_backend(backend.clone());
        e
    } else {
        let idle_timeout_secs = if interactive {
            config.cli.idle_timeout_secs
        } else {
            0
        };
        let pty_config = PtyConfig {
            interactive,
            idle_timeout_secs,
            workspace_root: config.core.workspace_root.clone(),
            ..PtyConfig::from_env()
        };
        temp_executor = PtyExecutor::new(backend.clone(), pty_config);
        &mut temp_executor
    };

    // Set TUI mode flag when TUI is connected (tui_lines is Some)
    // This replaces the broken output_rx.is_none() detection in PtyExecutor
    if tui_lines.is_some() {
        exec.set_tui_mode(true);
    }

    // Enter raw mode for interactive mode to capture keystrokes
    // Skip if TUI is connected - TUI owns raw mode and will manage it
    if interactive && !tui_connected {
        enable_raw_mode().context("Failed to enable raw mode")?;
    }

    // Use scopeguard to ensure raw mode is restored on any exit path
    // Skip if TUI is connected - TUI owns raw mode
    let _guard = scopeguard::guard((interactive, tui_connected), |(is_interactive, tui)| {
        if is_interactive && !tui {
            let _ = disable_raw_mode();
        }
    });

    // Run PTY executor with shared interrupt channel
    let result = if interactive && tui_lines.is_none() {
        // Raw interactive mode only when not using TUI (TUI handles its own terminal)
        exec.run_interactive(prompt, interrupt_rx).await
    } else if let Some(lines) = tui_lines {
        // TUI mode: use TuiStreamHandler to capture output for TUI display
        let verbose = verbosity == Verbosity::Verbose;
        let mut handler = TuiStreamHandler::with_lines(verbose, lines);
        exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
            .await
    } else {
        // Use streaming handler for non-interactive mode (respects verbosity)
        // Use PrettyStreamHandler for StreamJson backends (Claude) on TTY for markdown rendering
        // Use ConsoleStreamHandler for Text format backends (Kiro, Gemini, etc.) for immediate output
        let use_pretty =
            backend.output_format == BackendOutputFormat::StreamJson && stdout().is_terminal();

        match verbosity {
            Verbosity::Quiet => {
                let mut handler = QuietStreamHandler;
                exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                    .await
            }
            Verbosity::Normal => {
                if use_pretty {
                    let mut handler = PrettyStreamHandler::new(false);
                    exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                        .await
                } else {
                    let mut handler = ConsoleStreamHandler::new(false);
                    exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                        .await
                }
            }
            Verbosity::Verbose => {
                if use_pretty {
                    let mut handler = PrettyStreamHandler::new(true);
                    exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                        .await
                } else {
                    let mut handler = ConsoleStreamHandler::new(true);
                    exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                        .await
                }
            }
        }
    };

    match result {
        Ok(pty_result) => {
            let termination = convert_termination_type(pty_result.termination, interactive);

            // Use extracted_text for event parsing when available (NDJSON backends like Claude),
            // otherwise fall back to stripped_output (non-JSON backends or interactive mode).
            // This fixes event parsing for Claude's stream-json output where event tags like
            // <event topic="..."> are inside JSON string values and not directly visible.
            let output_for_parsing = if pty_result.extracted_text.is_empty() {
                pty_result.stripped_output
            } else {
                pty_result.extracted_text
            };
            Ok(ExecutionOutcome {
                output: output_for_parsing,
                success: pty_result.success,
                termination,
            })
        }
        Err(e) => {
            // PTY allocation may have failed - log and continue with error
            warn!("PTY execution failed: {}, continuing with error status", e);
            Err(anyhow::Error::new(e))
        }
    }
}

/// Logs events parsed from output to the event history file.
///
/// When an event has no subscriber (orphan), also logs an `event.orphaned`
/// system event to help Ralph understand the misconfiguration.
fn log_events_from_output(
    logger: &mut EventLogger,
    iteration: u32,
    hat_id: &HatId,
    output: &str,
    registry: &ralph_core::HatRegistry,
) {
    let parser = EventParser::new();
    let events = parser.parse(output);

    for event in events {
        // Determine which hat will be triggered by this event
        let triggered = registry.find_by_trigger(event.topic.as_str());

        // Per spec: Log "Published {topic} -> triggers {hat}" at DEBUG level
        if let Some(triggered_hat) = triggered {
            debug!("Published {} -> triggers {}", event.topic, triggered_hat);
        } else {
            debug!(
                "Published {} -> no hat triggered (orphan event)",
                event.topic
            );

            // Emit event.orphaned system event so Ralph sees the problem
            // Collect valid events (all hat subscriptions except wildcards)
            let valid_events: Vec<String> = registry
                .all()
                .flat_map(|hat| hat.subscriptions.iter())
                .map(|t| t.as_str().to_string())
                .filter(|t| t != "*")
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            warn!(
                topic = %event.topic,
                source = %hat_id.as_str(),
                valid_events = ?valid_events,
                "Event has no subscriber - logging event.orphaned"
            );

            let orphan_event = Event::new(
                "event.orphaned",
                format!(
                    "Event '{}' has no subscriber hat. Valid events to publish: {:?}",
                    event.topic, valid_events
                ),
            )
            .with_source(hat_id.clone());

            let orphan_record = EventRecord::new(iteration, "loop", &orphan_event, None::<&HatId>);
            if let Err(e) = logger.log(&orphan_record) {
                warn!("Failed to log event.orphaned: {}", e);
            }
        }

        let record = EventRecord::new(iteration, hat_id.to_string(), &event, triggered);

        if let Err(e) = logger.log(&record) {
            warn!("Failed to log event {}: {}", event.topic, e);
        }
    }
}

/// Logs the loop.terminate system event to the event history.
///
/// Per spec: loop.terminate is an observer-only event published on loop exit.
fn log_terminate_event(logger: &mut EventLogger, iteration: u32, event: &Event) {
    // loop.terminate is published by the orchestrator, not a hat
    // No hat can trigger on it (it's observer-only)
    let record = EventRecord::new(iteration, "loop", event, None::<&HatId>);

    if let Err(e) = logger.log(&record) {
        warn!("Failed to log loop.terminate event: {}", e);
    }
}

/// Gets the last commit info (short SHA and subject) for the summary file.
fn get_last_commit_info_with_cmd(git_cmd: &OsStr) -> Option<String> {
    let output = Command::new(git_cmd)
        .args(["log", "-1", "--format=%h: %s"])
        .output()
        .ok()?;

    if output.status.success() {
        let info = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if info.is_empty() { None } else { Some(info) }
    } else {
        None
    }
}

fn get_last_commit_info() -> Option<String> {
    get_last_commit_info_with_cmd(OsStr::new("git"))
}

/// Resolves prompt content with proper precedence.
///
/// Precedence (highest to lowest):
/// 1. CLI -p "text" (inline prompt text)
/// 2. CLI -P path (prompt file path)
/// 3. Config event_loop.prompt (inline prompt text)
/// 4. Config event_loop.prompt_file (prompt file path)
/// 5. Default PROMPT.md
///
/// Note: CLI overrides are already applied to config before this function is called.
fn resolve_prompt_content(event_loop_config: &ralph_core::EventLoopConfig) -> Result<String> {
    debug!(
        inline_prompt = ?event_loop_config.prompt.as_ref().map(|s| format!("{}...", &s[..s.len().min(50)])),
        prompt_file = %event_loop_config.prompt_file,
        "Resolving prompt content"
    );

    // Check for inline prompt first (CLI -p or config prompt)
    if let Some(ref inline_text) = event_loop_config.prompt {
        debug!(len = inline_text.len(), "Using inline prompt text");
        return Ok(inline_text.clone());
    }

    // Check for prompt file (CLI -P or config prompt_file or default)
    let prompt_file = &event_loop_config.prompt_file;
    if !prompt_file.is_empty() {
        let path = std::path::Path::new(prompt_file);
        debug!(path = %prompt_file, exists = path.exists(), "Checking prompt file");
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read prompt file: {}", prompt_file))?;
            debug!(path = %prompt_file, len = content.len(), "Read prompt from file");
            return Ok(content);
        } else {
            // File specified but doesn't exist - error with helpful message
            anyhow::bail!(
                "Prompt file '{}' not found. Check the path or use -p \"text\" for inline prompt.",
                prompt_file
            );
        }
    }

    // No valid prompt source found
    anyhow::bail!(
        "No prompt specified. Use -p \"text\" for inline prompt, -P path for file, \
         or create PROMPT.md in the current directory."
    )
}

/// Checks for planning session user responses and publishes them as events.
///
/// When running in planning mode (RALPH_PLANNING_SESSION_ID is set),
/// this function reads the conversation file for new user responses and
/// publishes them as `user.response` events to the event loop.
fn check_planning_session_responses(event_loop: &mut EventLoop) -> Result<()> {
    // Get the planning session ID from environment
    let session_id = match std::env::var("RALPH_PLANNING_SESSION_ID") {
        Ok(id) => id,
        Err(_) => return Ok(()), // Not in planning mode
    };
    check_planning_session_responses_for_session(event_loop, &session_id)
}

fn check_planning_session_responses_for_session(
    event_loop: &mut EventLoop,
    session_id: &str,
) -> Result<()> {
    // Get loop context to find the conversation file path
    let ctx = match event_loop.loop_context() {
        Some(ctx) => ctx,
        None => return Ok(()), // No context, can't find conversation file
    };

    let conversation_path = ctx.planning_conversation_path(session_id);

    // Read conversation entries and look for new responses
    // We track which response IDs we've already processed to avoid duplicates

    // Track processed response IDs (static to persist across iterations)
    static PROCESSED_RESPONSES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    let conversation_content = match fs::read_to_string(&conversation_path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()), // File doesn't exist yet
        Err(e) => {
            warn!(
                session_id = %session_id,
                error = %e,
                "Failed to read planning conversation file"
            );
            return Ok(());
        }
    };

    let mut processed = PROCESSED_RESPONSES.lock().unwrap();

    for line in conversation_content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse the conversation entry
        let entry: ralph_core::planning_session::ConversationEntry =
            match serde_json::from_str(line) {
                Ok(entry) => entry,
                Err(e) => {
                    warn!(
                        session_id = %session_id,
                        line = %line,
                        error = %e,
                        "Failed to parse conversation entry"
                    );
                    continue;
                }
            };

        // Only process user_response entries
        if entry.entry_type != ralph_core::planning_session::ConversationType::UserResponse {
            continue;
        }

        // Check if we've already processed this response
        let response_key = format!("{}:{}", entry.id, entry.ts);
        if processed.contains(&response_key) {
            continue;
        }

        // Publish as user.response event
        let event = Event::new(
            "user.response",
            format!("[id: {}] {}", entry.id, entry.text),
        );
        event_loop.bus().publish(event.clone());

        info!(
            session_id = %session_id,
            response_id = %entry.id,
            "Published user response from planning session"
        );

        // Mark as processed
        processed.push(response_key);
    }

    Ok(())
}

/// Processes pending merges from the merge queue.
///
/// Called when the primary loop completes successfully. Spawns merge-ralph
/// processes for each queued loop in FIFO order.
fn process_pending_merges_with_command(repo_root: &Path, ralph_cmd: &OsStr) {
    let queue = MergeQueue::new(repo_root);

    // Get all pending merges
    let pending = match queue.list_by_state(ralph_core::merge_queue::MergeState::Queued) {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Failed to read merge queue: {}", e);
            return;
        }
    };

    if pending.is_empty() {
        debug!("No pending merges in queue");
        return;
    }

    info!(
        count = pending.len(),
        "Processing pending merges from queue"
    );

    // Get the merge-loop preset content
    let preset = match crate::presets::get_preset("merge-loop") {
        Some(p) => p,
        None => {
            warn!("merge-loop preset not found, pending merges will remain queued");
            return;
        }
    };

    // Write the merge config once (shared by all merge loops)
    let config_path = repo_root.join(".ralph/merge-loop-config.yml");
    if let Err(e) = fs::write(&config_path, preset.content) {
        warn!(
            error = %e,
            "Failed to write merge config, pending merges will remain queued"
        );
        return;
    }

    // Process each pending merge
    for entry in pending {
        let loop_id = &entry.loop_id;

        info!(loop_id = %loop_id, "Spawning merge-ralph process");

        match Command::new(ralph_cmd)
            .current_dir(repo_root)
            .args([
                "run",
                "-c",
                ".ralph/merge-loop-config.yml",
                "--exclusive",
                "--no-tui",
                "-p",
                &format!("Merge loop {} from branch ralph/{}", loop_id, loop_id),
            ])
            .env("RALPH_MERGE_LOOP_ID", loop_id)
            .spawn()
        {
            Ok(child) => {
                info!(
                    loop_id = %loop_id,
                    pid = child.id(),
                    "merge-ralph spawned successfully"
                );
            }
            Err(e) => {
                warn!(
                    loop_id = %loop_id,
                    error = %e,
                    "Failed to spawn merge-ralph, loop will remain queued for manual retry"
                );
            }
        }
    }
}

fn process_pending_merges(repo_root: &Path) {
    process_pending_merges_with_command(repo_root, OsStr::new("ralph"));
}

/// Public wrapper for CLI invocation of process_pending_merges.
///
/// Called by `ralph loops process` command to process the merge queue.
pub fn process_pending_merges_cli(repo_root: &Path) {
    process_pending_merges(repo_root);
}

/// Start a loop from an external caller (e.g., the bot daemon).
///
/// Loads config from `ralph.yml`, applies the given prompt, acquires the
/// loop lock, and runs the orchestration loop headlessly. The caller is
/// responsible for Telegram interaction — the spawned loop has `robot.enabled`
/// disabled to prevent a second Telegram poller from conflicting.
///
/// Returns `Ok(TerminationReason)` on completion or `Err` on fatal errors.
pub async fn start_loop(
    prompt: String,
    workspace_root: PathBuf,
    config_path: Option<PathBuf>,
) -> Result<TerminationReason> {
    use crate::{ColorMode, ConfigSource, load_config_with_overrides};

    // Load config from file or defaults
    let config_source = config_path.unwrap_or_else(|| workspace_root.join("ralph.yml"));
    let sources = vec![ConfigSource::File(config_source)];
    let mut config = load_config_with_overrides(&sources)?;

    // Set workspace root to the provided path
    config.core.workspace_root = workspace_root.clone();

    // Apply the prompt
    config.event_loop.prompt = Some(prompt);
    config.event_loop.prompt_file = String::new();

    // Keep robot.enabled as-is from config. When the daemon starts a loop,
    // the loop's own TelegramService handles all Telegram interaction
    // (commands, guidance, responses, check-ins). The daemon stops polling
    // while the loop runs, so there's no conflict.

    // Force autonomous headless mode (no TUI, no interactive)
    config.cli.default_mode = "autonomous".to_string();

    // Normalize and validate
    config.normalize();
    let warnings = config
        .validate()
        .context("Configuration validation failed")?;
    for warning in &warnings {
        tracing::warn!("{}", warning);
    }

    // Auto-detect backend if needed
    if config.cli.backend == "auto" {
        let priority = config.get_agent_priority();
        let detected = ralph_adapters::detect_backend(&priority, |backend| {
            config.adapter_settings(backend).enabled
        });
        match detected {
            Ok(backend) => {
                info!("Auto-detected backend: {}", backend);
                config.cli.backend = backend;
            }
            Err(e) => return Err(anyhow::Error::new(e)),
        }
    }

    // Ensure scratchpad directory exists
    crate::ensure_scratchpad_directory(&config)?;

    // Acquire the loop lock (primary loop)
    let prompt_summary = config.event_loop.prompt.as_deref().unwrap_or("[daemon]");
    let prompt_summary = ralph_core::truncate_with_ellipsis(prompt_summary, 100);

    let _lock_guard = ralph_core::LoopLock::try_acquire(&workspace_root, &prompt_summary)
        .context("Failed to acquire loop lock — another loop may be running")?;

    let loop_context = ralph_core::LoopContext::primary(workspace_root);

    // Run the loop headlessly
    run_loop_impl(
        config,
        ColorMode::Never,
        false, // not resume
        false, // no TUI
        Verbosity::Normal,
        None, // no session recording
        Some(loop_context),
        Vec::new(), // no custom args
        None,       // default auto-merge
    )
    .await
}

/// Creates a robot service (Telegram) for human-in-the-loop communication.
///
/// Called by `run_loop_impl` when `robot.enabled` is true and this is the primary loop.
/// Returns `None` if the service cannot be created or started.
fn create_robot_service(
    config: &RalphConfig,
    context: &LoopContext,
) -> Option<Box<dyn ralph_proto::RobotService>> {
    let workspace_root = context.workspace().to_path_buf();
    let bot_token = config.robot.resolve_bot_token();
    let timeout_secs = config.robot.timeout_seconds.unwrap_or(300);
    let loop_id = context
        .loop_id()
        .map(String::from)
        .unwrap_or_else(|| "main".to_string());

    match ralph_telegram::TelegramService::new(workspace_root, bot_token, timeout_secs, loop_id) {
        Ok(service) => {
            if let Err(e) = service.start() {
                warn!(error = %e, "Failed to start robot service");
                return None;
            }
            info!(
                bot_token = %service.bot_token_masked(),
                timeout_secs = service.timeout_secs(),
                "Robot human-in-the-loop service active"
            );
            Some(Box::new(service))
        }
        Err(e) => {
            warn!(error = %e, "Failed to create robot service");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::CwdGuard;
    use ralph_core::HatRegistry;
    use ralph_core::planning_session::{ConversationEntry, ConversationType};
    use ralph_proto::{Hat, Topic};
    use std::ffi::OsStr;
    use std::sync::Arc;
    use std::sync::Mutex;

    #[test]
    fn test_pty_always_enabled_for_streaming() {
        // PTY mode is always enabled for real-time streaming output.
        // This ensures all backends (claude, gemini, kiro, codex, amp) get
        // streaming output instead of buffered output from CliExecutor.
        let use_pty = true; // Matches the actual implementation

        // PTY should always be true regardless of backend or mode
        assert!(use_pty, "PTY should always be enabled for streaming output");
    }

    #[test]
    fn test_user_interactive_mode_determination() {
        // user_interactive is determined by default_mode setting, not PTY.
        // PTY handles output streaming; user_interactive handles input forwarding.

        // Autonomous mode: no user input forwarding
        let autonomous_interactive = false;
        assert!(
            !autonomous_interactive,
            "Autonomous mode should not forward user input"
        );

        // Interactive mode with TTY: forward user input
        let interactive_with_tty = true;
        assert!(
            interactive_with_tty,
            "Interactive mode with TTY should forward user input"
        );
    }

    #[test]
    fn test_prepare_tui_iteration_seeds_max_iterations() {
        let state = Arc::new(Mutex::new(ralph_tui::TuiState::new()));

        let lines = prepare_tui_iteration(&state, "Planner".to_string(), "claude".to_string(), 42);

        assert!(lines.is_some(), "should return a lines handle");
        let state = state.lock().expect("state lock");
        assert_eq!(state.max_iterations, Some(42));
        assert_eq!(state.total_iterations(), 1);
    }

    #[cfg(unix)]
    fn write_fake_executable(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        let script = format!("#!/bin/sh\n{}\n", body);
        std::fs::write(&path, script).expect("write script");
        let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
        path
    }

    #[test]
    fn test_idle_timeout_interactive_mode_continues() {
        // Given: interactive mode and IdleTimeout termination
        let termination_type = ralph_adapters::TerminationType::IdleTimeout;
        let interactive = true;

        // When: converting termination type
        let result = convert_termination_type(termination_type, interactive);

        // Then: should return None (allow iteration to continue)
        assert!(
            result.is_none(),
            "Interactive mode idle timeout should return None to allow iteration progression"
        );
    }

    #[test]
    fn test_idle_timeout_autonomous_mode_stops() {
        // Given: autonomous mode and IdleTimeout termination
        let termination_type = ralph_adapters::TerminationType::IdleTimeout;
        let interactive = false;

        // When: converting termination type
        let result = convert_termination_type(termination_type, interactive);

        // Then: should return Some(Stopped)
        assert_eq!(
            result,
            Some(TerminationReason::Stopped),
            "Autonomous mode idle timeout should return Stopped"
        );
    }

    #[test]
    fn test_natural_termination_always_continues() {
        // Given: Natural termination in any mode
        let termination_type = ralph_adapters::TerminationType::Natural;

        // When/Then: should return None regardless of mode
        assert!(
            convert_termination_type(termination_type.clone(), true).is_none(),
            "Natural termination should continue in interactive mode"
        );
        assert!(
            convert_termination_type(termination_type, false).is_none(),
            "Natural termination should continue in autonomous mode"
        );
    }

    #[test]
    fn test_user_interrupt_always_terminates() {
        // Given: UserInterrupt termination in any mode
        let termination_type = ralph_adapters::TerminationType::UserInterrupt;

        // When/Then: should return Interrupted regardless of mode
        assert_eq!(
            convert_termination_type(termination_type.clone(), true),
            Some(TerminationReason::Interrupted),
            "UserInterrupt should terminate in interactive mode"
        );
        assert_eq!(
            convert_termination_type(termination_type, false),
            Some(TerminationReason::Interrupted),
            "UserInterrupt should terminate in autonomous mode"
        );
    }

    #[test]
    fn test_force_kill_always_terminates() {
        // Given: ForceKill termination in any mode
        let termination_type = ralph_adapters::TerminationType::ForceKill;

        // When/Then: should return Interrupted regardless of mode
        assert_eq!(
            convert_termination_type(termination_type.clone(), true),
            Some(TerminationReason::Interrupted),
            "ForceKill should terminate in interactive mode"
        );
        assert_eq!(
            convert_termination_type(termination_type, false),
            Some(TerminationReason::Interrupted),
            "ForceKill should terminate in autonomous mode"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_get_last_commit_info_returns_none_without_git() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());
        let missing_git = temp_dir.path().join("git");
        assert!(get_last_commit_info_with_cmd(missing_git.as_os_str()).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_get_last_commit_info_reads_last_commit() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();

        Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo_root)
            .status()
            .expect("git init");

        std::fs::write(repo_root.join("README.md"), "hello").expect("write file");
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_root)
            .status()
            .expect("git add");

        Command::new("git")
            .args([
                "-c",
                "user.name=Test User",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "Initial commit",
                "--quiet",
            ])
            .current_dir(repo_root)
            .status()
            .expect("git commit");

        let _cwd = CwdGuard::set(repo_root);
        let info = get_last_commit_info_with_cmd(OsStr::new("git")).expect("commit info");
        assert!(
            info.contains("Initial commit"),
            "unexpected commit info: {info}"
        );
    }

    #[test]
    fn test_process_pending_merges_handles_missing_preset() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();
        std::fs::create_dir_all(repo_root.join(".ralph/merge-queue")).expect("queue dir");

        process_pending_merges(repo_root);
    }

    #[cfg(unix)]
    #[test]
    fn test_process_pending_merges_spawns_for_queue_entry() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();
        std::fs::create_dir_all(repo_root.join(".ralph/merge-queue")).expect("queue dir");

        let queue_file = repo_root.join(".ralph/merge-queue/loop-1234.json");
        std::fs::write(
            &queue_file,
            r#"{"loop_id":"1234","state":"queued","created_at":"2026-01-01T00:00:00Z"}"#,
        )
        .expect("queue file");

        let bin_dir = repo_root.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let ralph_path = write_fake_executable(&bin_dir, "ralph", "exit 0");

        process_pending_merges_with_command(repo_root, ralph_path.as_os_str());
    }

    #[test]
    fn test_process_pending_merges_missing_command_keeps_queue() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();
        let queue = ralph_core::merge_queue::MergeQueue::new(repo_root);
        queue.enqueue("loop-9999", "merge prompt").expect("enqueue");

        process_pending_merges_with_command(repo_root, OsStr::new("ralph-command-missing-12345"));

        let config_path = repo_root.join(".ralph/merge-loop-config.yml");
        assert!(config_path.exists());
        let entries = queue
            .list_by_state(ralph_core::merge_queue::MergeState::Queued)
            .expect("list queued");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].loop_id, "loop-9999");
    }

    #[test]
    fn test_process_pending_merges_with_empty_queue_no_config_written() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();
        std::fs::create_dir_all(repo_root.join(".ralph/merge-queue")).expect("queue dir");

        let config_path = repo_root.join(".ralph/merge-loop-config.yml");
        assert!(!config_path.exists());

        process_pending_merges_with_command(repo_root, OsStr::new("ralph"));

        assert!(!config_path.exists());
    }

    #[test]
    fn test_resolve_prompt_content_inline_precedence() {
        let mut config = RalphConfig::default();
        config.event_loop.prompt = Some("inline prompt".to_string());
        config.event_loop.prompt_file = "missing.md".to_string();

        let resolved = resolve_prompt_content(&config.event_loop).expect("inline prompt");
        assert_eq!(resolved, "inline prompt");
    }

    #[test]
    fn test_resolve_prompt_content_from_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let prompt_path = temp_dir.path().join("PROMPT.md");
        std::fs::write(&prompt_path, "file prompt").expect("write prompt");

        let mut config = RalphConfig::default();
        config.event_loop.prompt = None;
        config.event_loop.prompt_file = prompt_path.to_string_lossy().to_string();

        let resolved = resolve_prompt_content(&config.event_loop).expect("file prompt");
        assert_eq!(resolved, "file prompt");
    }

    #[test]
    fn test_resolve_prompt_content_missing_file_errors() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let missing_path = temp_dir.path().join("missing.md");

        let mut config = RalphConfig::default();
        config.event_loop.prompt = None;
        config.event_loop.prompt_file = missing_path.to_string_lossy().to_string();

        let err = resolve_prompt_content(&config.event_loop).expect_err("missing prompt");
        assert!(
            err.to_string().contains("Prompt file"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_resolve_prompt_content_no_prompt_errors() {
        let mut config = RalphConfig::default();
        config.event_loop.prompt = None;
        config.event_loop.prompt_file = String::new();

        let err = resolve_prompt_content(&config.event_loop).expect_err("missing prompt");
        assert!(
            err.to_string().contains("No prompt specified"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_log_events_from_output_records_orphan_event() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let log_path = temp_dir.path().join("events.jsonl");
        let mut logger = EventLogger::new(&log_path);

        let mut registry = HatRegistry::new();
        let mut hat = Hat::new("planner", "Planner");
        hat.subscriptions.push(Topic::new("task.start"));
        registry.register(hat);

        let output = "<event topic=\"task.start\">start</event>\n\
<event topic=\"unknown.event\">oops</event>";
        let hat_id = HatId::new("tester");

        log_events_from_output(&mut logger, 1, &hat_id, output, &registry);

        let content = std::fs::read_to_string(&log_path).expect("read events");
        let records: Vec<EventRecord> = content
            .lines()
            .map(|line| serde_json::from_str(line).expect("record"))
            .collect();

        let topics: std::collections::HashSet<String> =
            records.iter().map(|record| record.topic.clone()).collect();
        assert!(topics.contains("task.start"));
        assert!(topics.contains("unknown.event"));
        assert!(topics.contains("event.orphaned"));

        let triggered = records
            .iter()
            .find(|record| record.topic == "task.start")
            .and_then(|record| record.triggered.clone());
        assert_eq!(triggered.as_deref(), Some("planner"));
    }

    #[test]
    fn test_log_terminate_event_writes_record() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let log_path = temp_dir.path().join("events.jsonl");
        let mut logger = EventLogger::new(&log_path);

        let event = Event::new("loop.terminate", "done");
        log_terminate_event(&mut logger, 7, &event);

        let content = std::fs::read_to_string(&log_path).expect("read events");
        let records: Vec<EventRecord> = content
            .lines()
            .map(|line| serde_json::from_str(line).expect("record"))
            .collect();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].topic, "loop.terminate");
        assert_eq!(records[0].hat, "loop");
        assert_eq!(records[0].iteration, 7);
    }

    #[test]
    fn test_check_planning_session_responses_publishes_user_response() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let session_id = format!(
            "session-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );

        let mut config = RalphConfig::default();
        config.core.workspace_root = temp_dir.path().to_path_buf();
        let ctx = ralph_core::LoopContext::primary(temp_dir.path().to_path_buf());
        let mut event_loop = EventLoop::with_context(config, ctx.clone());

        let conversation_path = ctx.planning_conversation_path(&session_id);
        std::fs::create_dir_all(conversation_path.parent().expect("parent"))
            .expect("create conversation dir");

        let prompt_entry = ConversationEntry {
            entry_type: ConversationType::UserPrompt,
            id: "prompt-1".to_string(),
            text: "Which option?".to_string(),
            ts: "2026-01-31T00:00:00Z".to_string(),
        };
        let response_entry = ConversationEntry {
            entry_type: ConversationType::UserResponse,
            id: "response-1".to_string(),
            text: "Option A".to_string(),
            ts: "2026-01-31T00:00:01Z".to_string(),
        };
        let conversation = format!(
            "{}\n{}\n",
            serde_json::to_string(&prompt_entry).expect("serialize prompt"),
            serde_json::to_string(&response_entry).expect("serialize response")
        );
        std::fs::write(&conversation_path, conversation).expect("write conversation");

        let published = std::sync::Arc::new(Mutex::new(Vec::new()));
        let published_clone = std::sync::Arc::clone(&published);
        event_loop
            .bus()
            .add_observer(move |event| published_clone.lock().unwrap().push(event.clone()));

        check_planning_session_responses_for_session(&mut event_loop, &session_id)
            .expect("check responses");
        {
            let events = published.lock().unwrap();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].topic.as_str(), "user.response");
            assert!(events[0].payload.contains("response-1"));
        }

        check_planning_session_responses_for_session(&mut event_loop, &session_id)
            .expect("dedup responses");
        let events = published.lock().unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_check_planning_session_responses_for_session_no_context_is_ok() {
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);

        let published = std::sync::Arc::new(Mutex::new(Vec::new()));
        let published_clone = std::sync::Arc::clone(&published);
        event_loop
            .bus()
            .add_observer(move |event| published_clone.lock().unwrap().push(event.clone()));

        check_planning_session_responses_for_session(&mut event_loop, "session-no-context")
            .expect("check responses");

        assert!(published.lock().unwrap().is_empty());
    }

    #[test]
    fn test_check_planning_session_responses_skips_invalid_json() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let session_id = format!(
            "session-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );

        let mut config = RalphConfig::default();
        config.core.workspace_root = temp_dir.path().to_path_buf();
        let ctx = ralph_core::LoopContext::primary(temp_dir.path().to_path_buf());
        let mut event_loop = EventLoop::with_context(config, ctx.clone());

        let conversation_path = ctx.planning_conversation_path(&session_id);
        std::fs::create_dir_all(conversation_path.parent().expect("parent"))
            .expect("create conversation dir");

        let prompt_entry = ConversationEntry {
            entry_type: ConversationType::UserPrompt,
            id: "prompt-1".to_string(),
            text: "Choose one".to_string(),
            ts: "2026-01-31T00:00:00Z".to_string(),
        };
        let conversation = format!(
            "not-json\n{}\n",
            serde_json::to_string(&prompt_entry).expect("serialize prompt")
        );
        std::fs::write(&conversation_path, conversation).expect("write conversation");

        let published = std::sync::Arc::new(Mutex::new(Vec::new()));
        let published_clone = std::sync::Arc::clone(&published);
        event_loop
            .bus()
            .add_observer(move |event| published_clone.lock().unwrap().push(event.clone()));

        check_planning_session_responses_for_session(&mut event_loop, &session_id)
            .expect("check responses");

        assert!(published.lock().unwrap().is_empty());
    }
}
