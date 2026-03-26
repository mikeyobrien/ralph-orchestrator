//! Core orchestration loop implementation.
//!
//! This module contains the main `run_loop_impl` function that executes
//! the Ralph orchestration loop, along with supporting types and helper
//! functions for PTY execution and termination handling.

use anyhow::{Context, Result};
use ralph_adapters::{
    AcpExecutor, ClaudeStreamEvent, ClaudeStreamParser, CliBackend, CliExecutor,
    ConsoleStreamHandler, ContentBlock, JsonRpcStreamHandler, OutputFormat as BackendOutputFormat,
    PiAssistantEvent, PiContentBlock, PiStreamEvent, PiStreamParser, PrettyStreamHandler,
    PtyConfig, PtyExecutor, QuietStreamHandler, StreamHandler, TuiStreamHandler,
};
use ralph_core::diagnostics::{HookDisposition, HookRunTelemetryEntry};
use ralph_core::{
    CompletionAction, EventLogger, EventLoop, EventParser, EventRecord, HatRegistry, HookEngine,
    HookExecutor, HookExecutorContract, HookMutationConfig, HookOnError, HookPayloadBuilderInput,
    HookPayloadContextInput, HookPhaseEvent, HookRunRequest, HookRunResult, HookSuspendMode,
    LoopCompletionHandler, LoopContext, LoopHistory, LoopRegistry, MergeQueue, RalphConfig, Record,
    SessionRecorder, SummaryWriter, SuspendStateRecord, SuspendStateStore, TerminationReason,
    UrgentSteerStore,
};
use ralph_proto::{Event, GuidanceTarget, HatId, RpcEvent, RpcState, RpcTaskCounts};
use ralph_tui::Tui;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufWriter, IsTerminal, stdin, stdout};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::display::{
    build_tui_hat_map, print_iteration_separator, print_termination, print_wave_header,
    print_wave_summary, print_wave_worker_done,
};
use crate::process_management;
use crate::rpc_stdin::{GuidanceMessage, RpcDispatcher, run_stdin_reader, run_stdout_emitter};
use crate::{ColorMode, Verbosity};

/// Outcome of executing a prompt via PTY or CLI executor.
pub(crate) struct ExecutionOutcome {
    pub output: String,
    pub success: bool,
    pub termination: Option<TerminationReason>,
    pub total_cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// Shared atomic state written by the main loop and read by the RPC `get_state` handler.
struct RpcSharedState {
    iteration: Arc<std::sync::atomic::AtomicU32>,
    /// Current (hat id, hat display name) pair.
    hat: Arc<std::sync::Mutex<(String, String)>>,
    completed: Arc<std::sync::atomic::AtomicBool>,
    total_cost_usd: Arc<std::sync::Mutex<f64>>,
}

/// Resolves the loop ID for task ownership tracking.
///
/// - Worktree loops: use the loop_id from the LoopContext.
/// - Primary loops (fresh): generate a new `primary-{timestamp}` ID.
/// - Primary loops (--continue): reuse the existing `current-loop-id` marker,
///   or use an explicit `--loop-id` if provided.
fn resolve_loop_id(
    ctx: &ralph_core::LoopContext,
    resume: bool,
    explicit_loop_id: Option<&str>,
) -> String {
    ctx.loop_id().map(|s| s.to_string()).unwrap_or_else(|| {
        if resume {
            if let Some(explicit_id) = explicit_loop_id {
                return explicit_id.to_string();
            }
            let marker = ctx.ralph_dir().join("current-loop-id");
            if let Ok(existing) = std::fs::read_to_string(&marker) {
                let existing = existing.trim().to_string();
                if !existing.is_empty() {
                    return existing;
                }
            }
        }
        // Fresh run: generate a new timestamped ID
        format!("primary-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"))
    })
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
/// * `resume_loop_id` - Explicit loop ID to use when resuming (`--loop-id`).
///   If `None` and `resume` is true, reuses the existing `current-loop-id` marker.
pub async fn run_loop_impl(
    config: RalphConfig,
    color_mode: ColorMode,
    resume: bool,
    enable_tui: bool,
    enable_rpc: bool,
    verbosity: Verbosity,
    record_session: Option<PathBuf>,
    loop_context: Option<LoopContext>,
    custom_args: Vec<String>,
    auto_merge_override: Option<bool>,
    resume_loop_id: Option<String>,
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
    // PTY is required for TUI/RPC observation and true interactive sessions.
    // Headless `ralph run --no-tui` should use CliExecutor so backends get their
    // non-interactive prompt forms (for example `claude -p` or `codex exec`).
    let use_pty = enable_tui || enable_rpc || user_interactive;

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
    let urgent_steer_path = ctx.urgent_steer_path();
    let urgent_steer_store = UrgentSteerStore::new(urgent_steer_path.clone());
    urgent_steer_store
        .clear()
        .context("Failed to clear stale urgent-steer marker")?;
    let _urgent_steer_cleanup = scopeguard::guard(urgent_steer_path.clone(), |path| {
        let _ = UrgentSteerStore::new(path).clear();
    });

    // Write loop ID to marker file for task ownership tracking.
    // For worktree loops, use the loop_id; for primary loops, generate one.
    // This file is read by `ralph tools task add` to tag new tasks.
    //
    // In --continue mode, reuse the existing loop ID so that tasks from the
    // previous run remain visible to `ralph tools task ready`. An explicit
    // --loop-id takes priority over the marker file.
    let loop_id = resolve_loop_id(&ctx, resume, resume_loop_id.as_deref());
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
        // Use the config's scratchpad path resolved against the workspace, not the
        // hardcoded LoopContext default, so custom paths are properly cleared
        let scratchpad_path = ctx.workspace().join(&config.core.scratchpad);
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

    let hooks_dispatch_enabled = config.hooks.enabled && !config.hooks.events.is_empty();
    let hook_engine = HookEngine::new(&config.hooks);
    let hook_executor = HookExecutor::new();
    let suspend_state_store = SuspendStateStore::new(ctx.workspace());
    let mut accumulated_hook_metadata = serde_json::Map::new();

    let pre_loop_start_outcomes = dispatch_phase_event_hooks(
        &event_loop,
        hooks_dispatch_enabled,
        &loop_id,
        &hook_engine,
        &hook_executor,
        HookPhaseEvent::PreLoopStart,
        build_loop_start_payload_input(
            &loop_id,
            &ctx,
            config.event_loop.max_iterations,
            event_loop.state().iteration,
            None,
            &accumulated_hook_metadata,
        ),
    );
    merge_accumulated_hook_metadata_from_outcomes(
        &mut accumulated_hook_metadata,
        &pre_loop_start_outcomes,
    );
    fail_if_blocking_loop_start_outcomes(&pre_loop_start_outcomes)?;
    let mut pending_suspend_termination_reason =
        wait_for_resume_if_suspended(&pre_loop_start_outcomes, &loop_id, &suspend_state_store)
            .await?;

    if pending_suspend_termination_reason.is_none() {
        // For resume mode, we initialize with a different event topic
        // This tells the planner to read existing scratchpad rather than creating a new one
        if resume {
            event_loop.initialize_resume(&prompt_content);
        } else {
            event_loop.initialize(&prompt_content);
        }

        let post_loop_start_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            hooks_dispatch_enabled,
            &loop_id,
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostLoopStart,
            build_loop_start_payload_input(
                &loop_id,
                &ctx,
                config.event_loop.max_iterations,
                event_loop.state().iteration,
                Some(event_loop.get_active_hat_id().as_str().to_string()),
                &accumulated_hook_metadata,
            ),
        );
        merge_accumulated_hook_metadata_from_outcomes(
            &mut accumulated_hook_metadata,
            &post_loop_start_outcomes,
        );
        fail_if_blocking_loop_start_outcomes(&post_loop_start_outcomes)?;
        pending_suspend_termination_reason =
            wait_for_resume_if_suspended(&post_loop_start_outcomes, &loop_id, &suspend_state_store)
                .await?;
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
    // Advance the event reader past the logged start event so it won't be
    // re-read by process_events_from_jsonl() — the start event is already
    // in the bus from initialize().
    event_loop.sync_event_reader_to_file_end();

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
        // In autonomous (non-interactive) mode, use a very wide PTY to prevent
        // line wrapping of long NDJSON output (Pi emits 800+ char JSON lines that
        // get garbled when the PTY wraps at 80 columns).
        let cols = if user_interactive {
            PtyConfig::from_env().cols
        } else {
            32768
        };
        let pty_config = PtyConfig {
            interactive: user_interactive,
            idle_timeout_secs,
            cols,
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
    let enable_tui = enable_tui && !enable_rpc && stdin().is_terminal() && stdout().is_terminal();

    // RPC mode state: channels for stdin commands and stdout events
    let (rpc_event_tx, rpc_event_rx) = if enable_rpc {
        let (tx, rx) = tokio::sync::mpsc::channel::<RpcEvent>(256);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    let (rpc_guidance_tx, mut rpc_guidance_rx) = if enable_rpc {
        let (tx, rx) = tokio::sync::mpsc::channel::<GuidanceMessage>(64);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    // Shared stdout writer for RPC mode (thread-safe for JsonRpcStreamHandler)
    let rpc_stdout: Option<Arc<std::sync::Mutex<std::io::Stdout>>> = if enable_rpc {
        Some(Arc::new(std::sync::Mutex::new(std::io::stdout())))
    } else {
        None
    };

    // RPC mode: spawn stdin reader and stdout emitter tasks
    let rpc_dispatcher_started = if enable_rpc {
        let backend_name = config.cli.backend.clone();
        let max_iters = config.event_loop.max_iterations;

        // Create shared state for get_state responses
        let rpc_state_iteration = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let rpc_state_hat: Arc<std::sync::Mutex<(String, String)>> = Arc::new(
            std::sync::Mutex::new(("unknown".to_string(), "Unknown".to_string())),
        );
        let rpc_state_completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let rpc_state_total_cost: Arc<std::sync::Mutex<f64>> = Arc::new(std::sync::Mutex::new(0.0));

        let rpc_state_iteration_clone = rpc_state_iteration.clone();
        let rpc_state_hat_clone = rpc_state_hat.clone();
        let rpc_state_completed_clone = rpc_state_completed.clone();
        let rpc_state_total_cost_clone = rpc_state_total_cost.clone();
        let rpc_state_started_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let state_fn = move || {
            let (hat, hat_display) = rpc_state_hat_clone
                .lock()
                .map(|g| g.clone())
                .unwrap_or_else(|_| ("unknown".to_string(), "Unknown".to_string()));
            let total_cost_usd = rpc_state_total_cost_clone.lock().map(|g| *g).unwrap_or(0.0);
            RpcState {
                iteration: rpc_state_iteration_clone.load(std::sync::atomic::Ordering::Relaxed),
                max_iterations: Some(max_iters),
                hat,
                hat_display,
                backend: backend_name.clone(),
                completed: rpc_state_completed_clone.load(std::sync::atomic::Ordering::Relaxed),
                started_at: rpc_state_started_at,
                iteration_started_at: None,
                task_counts: RpcTaskCounts::default(),
                active_task: None,
                total_cost_usd,
            }
        };

        let dispatcher = RpcDispatcher::new(
            interrupt_tx.clone(),
            rpc_guidance_tx
                .clone()
                .expect("RPC guidance tx should exist"),
            rpc_event_tx.clone().expect("RPC event tx should exist"),
            Some(urgent_steer_path.clone()),
            state_fn,
        );

        // Mark loop as started
        dispatcher.mark_loop_started();

        // Spawn stdin reader
        tokio::spawn(async move {
            run_stdin_reader(dispatcher, tokio::io::stdin()).await;
        });

        // Spawn stdout emitter
        let rx = rpc_event_rx.expect("RPC event rx should exist");
        tokio::spawn(async move {
            run_stdout_emitter(rx).await;
        });

        // Emit loop_started event
        if let Some(ref tx) = rpc_event_tx {
            let started_event = RpcEvent::LoopStarted {
                prompt: prompt_content.clone(),
                max_iterations: Some(config.event_loop.max_iterations),
                backend: config.cli.backend.clone(),
                started_at: rpc_state_started_at,
            };
            let _ = tx.try_send(started_event);
        }

        Some(RpcSharedState {
            iteration: rpc_state_iteration,
            hat: rpc_state_hat,
            completed: rpc_state_completed,
            total_cost_usd: rpc_state_total_cost,
        })
    } else {
        None
    };

    let (mut tui_handle, tui_state, guidance_next_queue) = if enable_tui {
        // Build hat map for dynamic topic-to-hat resolution
        // This allows TUI to display custom hats (e.g., "Security Reviewer")
        // instead of generic "ralph" for all events
        let hat_map = build_tui_hat_map(event_loop.registry());
        let tui = Tui::new()
            .with_hat_map(hat_map)
            .with_termination_signal(terminated_rx)
            .with_events_path(resolve_current_events_path(&ctx))
            .with_urgent_steer_path(urgent_steer_path.clone());

        // Get shared state and guidance queue before spawning (for content streaming)
        let state = tui.state();
        let guidance_queue = tui.guidance_next_queue();

        // Wire interrupt channel so TUI can signal main loop on Ctrl+C
        // (raw mode prevents SIGINT from being generated by the OS)
        let tui = tui.with_interrupt_tx(interrupt_tx.clone());

        let observer = tui.observer();
        event_loop.add_observer(observer);
        (
            Some(tokio::spawn(async move { tui.run().await })),
            Some(state),
            Some(guidance_queue),
        )
    } else {
        (None, None, None)
    };

    // Add RPC EventBus observer to map ralph_proto::Event topics to RpcEvent variants
    // Per Task 04 requirement #4: "Add an EventBus observer that serializes Event → RpcEvent"
    if let Some(ref tx) = rpc_event_tx {
        let tx_clone = tx.clone();
        event_loop.add_observer(move |event: &Event| {
            // Map all event topics to RpcEvent::OrchestrationEvent
            // This provides observability for: build.task, build.done, loop.terminate,
            // task.start, task.resume, and any custom hat events
            let rpc_event = RpcEvent::OrchestrationEvent {
                topic: event.topic.as_str().to_string(),
                payload: event.payload.clone(),
                source: event.source.as_ref().map(|h| h.as_str().to_string()),
                target: event.target.as_ref().map(|h| h.as_str().to_string()),
            };
            let _ = tx_clone.try_send(rpc_event);
        });
    }

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
                TerminationReason::LoopStale => "loop_stale",
                TerminationReason::ValidationFailure => "validation_failure",
                TerminationReason::Stopped => "stopped",
                TerminationReason::Interrupted => "interrupted",
                TerminationReason::RestartRequested => "restart_requested",
                TerminationReason::WorkspaceGone => "workspace_gone",
                TerminationReason::Cancelled => "cancelled",
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
                    TerminationReason::LoopStale => "stale loop detected",
                    TerminationReason::ValidationFailure => "validation failure",
                    TerminationReason::Stopped => "manually stopped",
                    TerminationReason::Interrupted => "interrupted by signal",
                    TerminationReason::CompletionPromise => unreachable!(),
                    TerminationReason::RestartRequested => "restart requested",
                    TerminationReason::WorkspaceGone => "workspace directory removed",
                    TerminationReason::Cancelled => "cancelled by human",
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
        // Skip in RPC mode - JSON events replace console output
        if !enable_tui && !enable_rpc {
            print_termination(reason, state, use_colors);
        }

        // Mark RPC state as completed so get_state reflects termination
        if let Some(ref shared) = rpc_dispatcher_started {
            shared
                .completed
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

        // Emit RPC loop_terminated event
        if let Some(ref tx) = rpc_event_tx {
            let terminated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            let rpc_reason = match reason {
                TerminationReason::CompletionPromise => {
                    ralph_proto::json_rpc::TerminationReason::Completed
                }
                TerminationReason::MaxIterations => {
                    ralph_proto::json_rpc::TerminationReason::MaxIterations
                }
                TerminationReason::Interrupted | TerminationReason::Stopped => {
                    ralph_proto::json_rpc::TerminationReason::Interrupted
                }
                _ => ralph_proto::json_rpc::TerminationReason::Error,
            };

            let accumulated_cost = rpc_dispatcher_started
                .as_ref()
                .and_then(|s| s.total_cost_usd.lock().ok().map(|g| *g))
                .unwrap_or(0.0);

            let terminate_event = RpcEvent::LoopTerminated {
                reason: rpc_reason,
                total_iterations: state.iteration,
                duration_ms: state.elapsed().as_millis() as u64,
                total_cost_usd: accumulated_cost,
                terminated_at,
            };
            let _ = tx.try_send(terminate_event);
        }
    };

    if let Some(reason) = pending_suspend_termination_reason.take() {
        let reason = dispatch_pre_loop_termination_hooks(
            &event_loop,
            hooks_dispatch_enabled,
            &loop_id,
            &hook_engine,
            &hook_executor,
            &suspend_state_store,
            &ctx,
            config.event_loop.max_iterations,
            &mut accumulated_hook_metadata,
            reason,
        )
        .await?;

        let terminate_event = event_loop.publish_terminate_event(&reason);
        log_terminate_event(
            &mut event_logger,
            event_loop.state().iteration,
            &terminate_event,
        );

        let reason = dispatch_post_loop_termination_hooks(
            &event_loop,
            hooks_dispatch_enabled,
            &loop_id,
            &hook_engine,
            &hook_executor,
            &suspend_state_store,
            &ctx,
            config.event_loop.max_iterations,
            &mut accumulated_hook_metadata,
            reason,
        )
        .await?;

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
            let reason = dispatch_pre_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                TerminationReason::Interrupted,
            )
            .await?;

            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );

            let reason = dispatch_post_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

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

        // Drain next-loop guidance queue and write as human.guidance events.
        // These will be picked up by process_events_from_jsonl() during build_prompt().
        // Handle both TUI guidance queue and RPC guidance channel.
        let mut guidance_messages: Vec<String> = Vec::new();

        // Drain TUI guidance queue
        if let Some(ref queue) = guidance_next_queue {
            let messages: Vec<String> = {
                let mut q = queue.lock().unwrap();
                q.drain(..).collect()
            };
            guidance_messages.extend(messages);
        }

        // Drain RPC guidance channel (non-blocking)
        if let Some(ref mut rx) = rpc_guidance_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg.target {
                    GuidanceTarget::Current => {
                        debug!("Received RPC steer(current); applying at next prompt boundary");
                        guidance_messages.push(msg.message);
                    }
                    GuidanceTarget::Next => guidance_messages.push(msg.message),
                }
            }
        }

        if !guidance_messages.is_empty() {
            let events_path = resolve_current_events_path(&ctx);

            use std::io::Write;
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&events_path);

            let mut writer = match file {
                Ok(f) => std::io::BufWriter::new(f),
                Err(e) => {
                    warn!(error = %e, path = ?events_path, "Failed to open events file for guidance flush");
                    // Skip flushing - keep loop running
                    continue;
                }
            };

            for msg in &guidance_messages {
                let timestamp = chrono::Utc::now().to_rfc3339();
                let event = serde_json::json!({
                    "topic": "human.guidance",
                    "payload": msg,
                    "ts": timestamp,
                });

                match serde_json::to_string(&event) {
                    Ok(line) => {
                        if writeln!(writer, "{}", line).is_err() {
                            warn!(path = ?events_path, "Failed writing guidance event line");
                            break;
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed serializing guidance event");
                    }
                }
            }
            info!(
                count = guidance_messages.len(),
                "Wrote guidance events to events.jsonl"
            );
        }

        // Check termination before execution
        if let Some(reason) = event_loop.check_termination() {
            let reason = dispatch_pre_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

            // Per spec: Publish loop.terminate event to observers
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );

            let reason = dispatch_post_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

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

        let iteration = event_loop.state().iteration + 1;

        if event_loop.has_pending_events() {
            let pre_iteration_start_outcomes = dispatch_phase_event_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                HookPhaseEvent::PreIterationStart,
                build_iteration_start_payload_input(
                    &loop_id,
                    &ctx,
                    config.event_loop.max_iterations,
                    iteration,
                    Some(event_loop.get_active_hat_id().as_str().to_string()),
                    None,
                    None,
                    &accumulated_hook_metadata,
                ),
            );
            merge_accumulated_hook_metadata_from_outcomes(
                &mut accumulated_hook_metadata,
                &pre_iteration_start_outcomes,
            );
            fail_if_blocking_iteration_start_outcomes(&pre_iteration_start_outcomes)?;

            if let Some(reason) = wait_for_resume_if_suspended(
                &pre_iteration_start_outcomes,
                &loop_id,
                &suspend_state_store,
            )
            .await?
            {
                let reason = dispatch_pre_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(
                    &mut event_logger,
                    event_loop.state().iteration,
                    &terminate_event,
                );

                let reason = dispatch_post_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

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
        }

        // Get next hat to execute, with fallback recovery if no pending events
        let hat_id = match event_loop.next_hat() {
            Some(id) => {
                // Reset fallback counter on successful event routing
                consecutive_fallbacks = 0;
                id.clone()
            }
            None => {
                match recover_late_events_before_fallback(&mut event_loop)
                    .inspect_err(
                        |e| warn!(error = %e, "Failed to drain late JSONL events before fallback"),
                    )
                    .ok()
                {
                    Some(LateEventRecovery::PendingWork) => {
                        debug!(
                            "Recovered late JSONL events before fallback; retrying hat selection"
                        );
                        consecutive_fallbacks = 0;
                        continue;
                    }
                    Some(LateEventRecovery::Terminate(reason)) => {
                        let reason = dispatch_pre_loop_termination_hooks(
                            &event_loop,
                            hooks_dispatch_enabled,
                            &loop_id,
                            &hook_engine,
                            &hook_executor,
                            &suspend_state_store,
                            &ctx,
                            config.event_loop.max_iterations,
                            &mut accumulated_hook_metadata,
                            reason,
                        )
                        .await?;

                        let terminate_event = event_loop.publish_terminate_event(&reason);
                        log_terminate_event(
                            &mut event_logger,
                            event_loop.state().iteration,
                            &terminate_event,
                        );

                        let reason = dispatch_post_loop_termination_hooks(
                            &event_loop,
                            hooks_dispatch_enabled,
                            &loop_id,
                            &hook_engine,
                            &hook_executor,
                            &suspend_state_store,
                            &ctx,
                            config.event_loop.max_iterations,
                            &mut accumulated_hook_metadata,
                            reason,
                        )
                        .await?;

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
                    Some(LateEventRecovery::NoLateEvents) | None => {}
                }

                // No pending events - try to recover by injecting a fallback event
                // This triggers the built-in planner to assess the situation
                consecutive_fallbacks += 1;

                if consecutive_fallbacks > MAX_FALLBACK_ATTEMPTS {
                    warn!(
                        attempts = consecutive_fallbacks,
                        "Fallback recovery exhausted after {} attempts, terminating",
                        MAX_FALLBACK_ATTEMPTS
                    );
                    let reason = dispatch_pre_loop_termination_hooks(
                        &event_loop,
                        hooks_dispatch_enabled,
                        &loop_id,
                        &hook_engine,
                        &hook_executor,
                        &suspend_state_store,
                        &ctx,
                        config.event_loop.max_iterations,
                        &mut accumulated_hook_metadata,
                        TerminationReason::Stopped,
                    )
                    .await?;

                    let terminate_event = event_loop.publish_terminate_event(&reason);
                    log_terminate_event(
                        &mut event_logger,
                        event_loop.state().iteration,
                        &terminate_event,
                    );

                    let reason = dispatch_post_loop_termination_hooks(
                        &event_loop,
                        hooks_dispatch_enabled,
                        &loop_id,
                        &hook_engine,
                        &hook_executor,
                        &suspend_state_store,
                        &ctx,
                        config.event_loop.max_iterations,
                        &mut accumulated_hook_metadata,
                        reason,
                    )
                    .await?;

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
                let reason = dispatch_pre_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    TerminationReason::Stopped,
                )
                .await?;

                // Per spec: Publish loop.terminate event to observers
                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(
                    &mut event_logger,
                    event_loop.state().iteration,
                    &terminate_event,
                );

                let reason = dispatch_post_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

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

        // Update RPC state iteration counter
        if let Some(ref shared) = rpc_dispatcher_started {
            shared
                .iteration
                .store(iteration, std::sync::atomic::Ordering::Relaxed);
        }

        // Determine which hat to display in iteration separator
        // When Ralph is coordinating (hat_id == "ralph"), show the active hat being worked on
        let preview_display_hat = if hat_id.as_str() == "ralph" {
            event_loop.get_active_hat_id()
        } else {
            hat_id.clone()
        };

        let post_iteration_start_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            hooks_dispatch_enabled,
            &loop_id,
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostIterationStart,
            build_iteration_start_payload_input(
                &loop_id,
                &ctx,
                config.event_loop.max_iterations,
                iteration,
                Some(preview_display_hat.as_str().to_string()),
                Some(preview_display_hat.as_str().to_string()),
                None,
                &accumulated_hook_metadata,
            ),
        );
        merge_accumulated_hook_metadata_from_outcomes(
            &mut accumulated_hook_metadata,
            &post_iteration_start_outcomes,
        );
        fail_if_blocking_iteration_start_outcomes(&post_iteration_start_outcomes)?;

        if let Some(reason) = wait_for_resume_if_suspended(
            &post_iteration_start_outcomes,
            &loop_id,
            &suspend_state_store,
        )
        .await?
        {
            let reason = dispatch_pre_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );

            let reason = dispatch_post_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

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

        // Log hat changes with appropriate messaging
        // Skip in TUI mode - TUI shows hat info in header, and stdout would corrupt display
        // Skip in RPC mode - JSON events replace console output
        if last_hat.as_ref() != Some(&hat_id) {
            if tui_state.is_none() && !enable_rpc {
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

        let display_hat =
            resolve_display_hat_for_execution(&event_loop, &hat_id, &preview_display_hat);

        // Log full prompt to diagnostics (RALPH_DIAGNOSTICS=1)
        event_loop.log_prompt(iteration, display_hat.as_str(), &prompt);

        let hat_display = event_loop
            .registry()
            .get(&display_hat)
            .map(|hat| hat.name.clone())
            .unwrap_or_else(|| display_hat.as_str().to_string());

        // Update RPC shared hat state so get_state reflects the current iteration's hat.
        if let Some(ref shared) = rpc_dispatcher_started
            && let Ok(mut guard) = shared.hat.lock()
        {
            *guard = (display_hat.as_str().to_string(), hat_display.clone());
        }

        // Track iteration start time for RPC iteration_end duration calculation
        // (cheap to create even when not in RPC mode)
        let iteration_started_at = std::time::Instant::now();

        // Emit RPC iteration_start event after prompt construction so the displayed
        // hat matches the one actually selected for execution.
        if let Some(ref tx) = rpc_event_tx {
            let started_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let start_event = RpcEvent::IterationStart {
                iteration,
                max_iterations: Some(config.event_loop.max_iterations),
                hat: display_hat.as_str().to_string(),
                hat_display: hat_display.clone(),
                backend: config.cli.backend.clone(),
                started_at,
            };
            let _ = tx.try_send(start_event);
        }

        // Per spec: Print iteration demarcation separator
        // "Each iteration must be clearly demarcated in the output so users can
        // visually distinguish where one iteration ends and another begins."
        // Skip when TUI is enabled - TUI has its own header showing iteration info
        // Skip in RPC mode - JSON events replace console output
        if tui_state.is_none() && !enable_rpc {
            print_iteration_separator(
                iteration,
                display_hat.as_str(),
                event_loop.state().elapsed(),
                config.event_loop.max_iterations,
                use_colors,
            );
        }

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
        let hat_config_opt = event_loop.registry().get_config(&display_hat);
        let hat_backend_opt = hat_config_opt.and_then(|c| c.backend.as_ref());
        let hat_backend_args = hat_config_opt.and_then(|c| c.backend_args.clone());

        // Step 2: Resolve effective backend and determine backend name for timeout
        // Note: backend_name_for_timeout is owned String to avoid lifetime issues with hat_backend reference
        let (mut effective_backend, backend_name_for_timeout): (CliBackend, String) =
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
                                ralph_core::HatBackend::KiroAgent { backend_type, .. } => {
                                    backend_type.clone()
                                }
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

        // Step 2.5: Apply custom hat backend args if configured
        if let Some(args) = hat_backend_args {
            effective_backend.args.extend(args);
        }

        // Step 3: Get timeout from config based on actual backend being used
        let timeout_secs = config.adapter_settings(&backend_name_for_timeout).timeout;
        let timeout = Some(Duration::from_secs(timeout_secs));

        // For TUI mode, get the shared lines buffer for this iteration.
        // The buffer is owned by TuiState's IterationBuffer, so writes from
        // TuiStreamHandler appear immediately in the TUI (real-time streaming).
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
        let rpc_stdout_for_pty = rpc_stdout.clone();
        let execute_future = async {
            if effective_backend.output_format == BackendOutputFormat::Acp {
                execute_acp(
                    &effective_backend,
                    &config,
                    &prompt,
                    verbosity,
                    tui_lines_for_pty,
                    rpc_stdout_for_pty,
                    iteration,
                    display_hat.as_str(),
                    &backend_name_for_timeout,
                )
                .await
            } else if use_pty {
                execute_pty(
                    pty_executor.as_mut(),
                    &effective_backend,
                    &config,
                    &prompt,
                    user_interactive,
                    interrupt_rx_for_pty,
                    verbosity,
                    tui_lines_for_pty,
                    rpc_stdout_for_pty,
                    iteration,
                    display_hat.as_str(),
                    &backend_name_for_timeout,
                )
                .await
            } else {
                let executor = CliExecutor::new(effective_backend.clone());
                let result = executor
                    .execute(&prompt, stdout(), timeout, verbosity == Verbosity::Verbose)
                    .await?;
                Ok(ExecutionOutcome {
                    output: normalize_cli_output_for_parsing(
                        effective_backend.output_format,
                        &result.output,
                    ),
                    success: result.success,
                    termination: None,
                    total_cost_usd: 0.0,
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
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

                let reason = dispatch_pre_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    TerminationReason::Interrupted,
                )
                .await?;

                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(&mut event_logger, event_loop.state().iteration, &terminate_event);

                let reason = dispatch_post_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

                handle_termination(&reason, event_loop.state(), &config.core.scratchpad, &loop_history, &loop_context, auto_merge, &prompt_content);
                // Signal TUI to exit immediately on interrupt
                let _ = terminated_tx.send(true);
                return Ok(reason);
            }
        };

        if let Some(reason) = outcome.termination {
            let reason = dispatch_pre_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );

            let reason = dispatch_post_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

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

        // Emit RPC iteration_end event
        if let Some(ref tx) = rpc_event_tx {
            let duration_ms = iteration_started_at.elapsed().as_millis() as u64;
            // Check if this iteration's output contains LOOP_COMPLETE
            let loop_complete_triggered = output.contains(&config.event_loop.completion_promise);
            let iteration_cost_usd = outcome.total_cost_usd;
            if let Some(ref shared) = rpc_dispatcher_started
                && let Ok(mut guard) = shared.total_cost_usd.lock()
            {
                *guard += iteration_cost_usd;
            }
            let end_event = RpcEvent::IterationEnd {
                iteration,
                duration_ms,
                cost_usd: iteration_cost_usd,
                input_tokens: outcome.input_tokens,
                output_tokens: outcome.output_tokens,
                cache_read_tokens: outcome.cache_read_tokens,
                cache_write_tokens: outcome.cache_write_tokens,
                loop_complete_triggered,
            };
            let _ = tx.try_send(end_event);
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

            let reason = dispatch_pre_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

            // Per spec: Publish loop.terminate event to observers
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );

            let reason = dispatch_post_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

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

        let should_dispatch_plan_created_hooks = event_loop
            .has_pending_plan_events_in_jsonl()
            .inspect_err(|e| {
                warn!(
                    error = %e,
                    "Failed to inspect unread JSONL events for semantic plan.* topics"
                )
            })
            .unwrap_or(false);

        if should_dispatch_plan_created_hooks {
            let pre_plan_created_outcomes = dispatch_phase_event_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                HookPhaseEvent::PrePlanCreated,
                build_plan_created_payload_input(
                    &loop_id,
                    &ctx,
                    config.event_loop.max_iterations,
                    event_loop.state().iteration,
                    Some(display_hat.as_str().to_string()),
                    Some(display_hat.as_str().to_string()),
                    None,
                    &accumulated_hook_metadata,
                ),
            );
            merge_accumulated_hook_metadata_from_outcomes(
                &mut accumulated_hook_metadata,
                &pre_plan_created_outcomes,
            );
            fail_if_blocking_plan_created_outcomes(&pre_plan_created_outcomes)?;

            if let Some(reason) = wait_for_resume_if_suspended(
                &pre_plan_created_outcomes,
                &loop_id,
                &suspend_state_store,
            )
            .await?
            {
                let reason = dispatch_pre_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(
                    &mut event_logger,
                    event_loop.state().iteration,
                    &terminate_event,
                );

                let reason = dispatch_post_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

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
        }

        let pending_human_interact_context = event_loop
            .pending_human_interact_context_in_jsonl()
            .inspect_err(|e| {
                warn!(
                    error = %e,
                    "Failed to inspect unread JSONL events for human.interact boundary"
                )
            })
            .ok()
            .flatten();

        if let Some(human_interact_context) = pending_human_interact_context {
            let pre_human_interact_outcomes = dispatch_phase_event_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                HookPhaseEvent::PreHumanInteract,
                build_human_interact_payload_input(
                    &loop_id,
                    &ctx,
                    config.event_loop.max_iterations,
                    event_loop.state().iteration,
                    Some(display_hat.as_str().to_string()),
                    Some(display_hat.as_str().to_string()),
                    None,
                    Some(human_interact_context),
                    &accumulated_hook_metadata,
                ),
            );
            merge_accumulated_hook_metadata_from_outcomes(
                &mut accumulated_hook_metadata,
                &pre_human_interact_outcomes,
            );
            fail_if_blocking_human_interact_outcomes(&pre_human_interact_outcomes)?;

            if let Some(reason) = wait_for_resume_if_suspended(
                &pre_human_interact_outcomes,
                &loop_id,
                &suspend_state_store,
            )
            .await?
            {
                let reason = dispatch_pre_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(
                    &mut event_logger,
                    event_loop.state().iteration,
                    &terminate_event,
                );

                let reason = dispatch_post_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

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
        }

        // Read events from JSONL, partitioning wave events from regular events
        let (processed_events, wave_events) =
            match event_loop.process_events_from_jsonl_with_waves() {
                Ok(result) => (Some(result.processed), result.wave_events),
                Err(e) => {
                    warn!(error = %e, "Failed to read events from JSONL");
                    (None, Vec::new())
                }
            };

        if let Some(human_interact_context) = processed_events
            .as_ref()
            .and_then(|events| events.human_interact_context.clone())
        {
            let post_human_interact_outcomes = dispatch_phase_event_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                HookPhaseEvent::PostHumanInteract,
                build_human_interact_payload_input(
                    &loop_id,
                    &ctx,
                    config.event_loop.max_iterations,
                    event_loop.state().iteration,
                    Some(display_hat.as_str().to_string()),
                    Some(display_hat.as_str().to_string()),
                    None,
                    Some(human_interact_context),
                    &accumulated_hook_metadata,
                ),
            );
            merge_accumulated_hook_metadata_from_outcomes(
                &mut accumulated_hook_metadata,
                &post_human_interact_outcomes,
            );
            fail_if_blocking_human_interact_outcomes(&post_human_interact_outcomes)?;

            if let Some(reason) = wait_for_resume_if_suspended(
                &post_human_interact_outcomes,
                &loop_id,
                &suspend_state_store,
            )
            .await?
            {
                let reason = dispatch_pre_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(
                    &mut event_logger,
                    event_loop.state().iteration,
                    &terminate_event,
                );

                let reason = dispatch_post_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

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
        }

        if processed_events
            .as_ref()
            .map(|events| events.had_plan_events)
            .unwrap_or(false)
        {
            let post_plan_created_outcomes = dispatch_phase_event_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                HookPhaseEvent::PostPlanCreated,
                build_plan_created_payload_input(
                    &loop_id,
                    &ctx,
                    config.event_loop.max_iterations,
                    event_loop.state().iteration,
                    Some(display_hat.as_str().to_string()),
                    Some(display_hat.as_str().to_string()),
                    None,
                    &accumulated_hook_metadata,
                ),
            );
            merge_accumulated_hook_metadata_from_outcomes(
                &mut accumulated_hook_metadata,
                &post_plan_created_outcomes,
            );
            fail_if_blocking_plan_created_outcomes(&post_plan_created_outcomes)?;

            if let Some(reason) = wait_for_resume_if_suspended(
                &post_plan_created_outcomes,
                &loop_id,
                &suspend_state_store,
            )
            .await?
            {
                let reason = dispatch_pre_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(
                    &mut event_logger,
                    event_loop.state().iteration,
                    &terminate_event,
                );

                let reason = dispatch_post_loop_termination_hooks(
                    &event_loop,
                    hooks_dispatch_enabled,
                    &loop_id,
                    &hook_engine,
                    &hook_executor,
                    &suspend_state_store,
                    &ctx,
                    config.event_loop.max_iterations,
                    &mut accumulated_hook_metadata,
                    reason,
                )
                .await?;

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
        }

        let mut agent_wrote_events = processed_events
            .as_ref()
            .map(|events| events.had_events)
            .unwrap_or(false);

        if !agent_wrote_events && output_mentions_ralph_emit(&output) {
            match recover_expected_emit_after_output(&mut event_loop)
                .inspect_err(|e| warn!(error = %e, "Failed to recover expected emit events"))
                .ok()
            {
                Some(LateEventRecovery::PendingWork | LateEventRecovery::Terminate(_)) => {
                    agent_wrote_events = true;
                }
                Some(LateEventRecovery::NoLateEvents) | None => {
                    warn!(
                        hat = %hat_id.as_str(),
                        "Output indicated `ralph emit`, but no event became readable before fallback logic"
                    );
                }
            }
        }

        // Execute wave if wave events detected
        if !wave_events.is_empty() {
            handle_wave_events(
                &wave_events,
                &mut event_loop,
                &backend,
                &ctx,
                use_colors,
                enable_rpc,
                rpc_event_tx.as_ref(),
                tui_state.as_ref(),
            )
            .await;
        }

        // Inject default_publishes for active hats only when agent wrote no events.
        // Prefer the displayed execution hat first so a non-emitting turn still
        // falls back to the hat the user actually saw in the banner.
        if !agent_wrote_events && wave_events.is_empty() {
            let mut fallback_hats = Vec::new();
            if display_hat.as_str() != "ralph" {
                fallback_hats.push(display_hat.clone());
            }
            for active_hat_id in event_loop.state().last_active_hat_ids.clone() {
                if !fallback_hats.contains(&active_hat_id) {
                    fallback_hats.push(active_hat_id);
                }
            }

            for active_hat_id in &fallback_hats {
                event_loop.check_default_publishes(active_hat_id);
                if event_loop.has_pending_events() {
                    break; // One default is sufficient
                }
            }
        }

        // Check cancellation first (no chain validation) — takes priority over completion
        if let Some(reason) = event_loop.check_cancellation_event() {
            info!("Loop cancelled gracefully via loop.cancel event.");

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

        if let Some(reason) = event_loop.check_completion_event() {
            info!(
                "Completion event {} detected.",
                config.event_loop.completion_promise
            );

            let reason = dispatch_pre_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );

            let reason = dispatch_post_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

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

        if detect_solo_output_completion(
            event_loop.registry(),
            &output,
            &config.event_loop.completion_promise,
        ) {
            let reason = TerminationReason::CompletionPromise;
            info!(
                "All done! {} detected in solo output.",
                config.event_loop.completion_promise
            );

            let reason = dispatch_pre_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );

            let reason = dispatch_post_loop_termination_hooks(
                &event_loop,
                hooks_dispatch_enabled,
                &loop_id,
                &hook_engine,
                &hook_executor,
                &suspend_state_store,
                &ctx,
                config.event_loop.max_iterations,
                &mut accumulated_hook_metadata,
                reason,
            )
            .await?;

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

#[derive(Debug, Clone, PartialEq, Eq)]
enum LateEventRecovery {
    NoLateEvents,
    PendingWork,
    Terminate(TerminationReason),
}

const LATE_EVENT_RECOVERY_MAX_POLLS: u32 = 5;
const LATE_EVENT_RECOVERY_POLL_INTERVAL_MS: u64 = 50;
const EMIT_RECOVERY_MAX_POLLS: u32 = 20;
const EMIT_RECOVERY_POLL_INTERVAL_MS: u64 = 250;

fn poll_for_late_events(
    event_loop: &mut EventLoop,
    max_polls: u32,
    poll_interval_ms: u64,
) -> std::io::Result<LateEventRecovery> {
    for poll_attempt in 0..=max_polls {
        let processed = event_loop.process_events_from_jsonl()?;

        if let Some(reason) = event_loop.check_cancellation_event() {
            return Ok(LateEventRecovery::Terminate(reason));
        }

        if let Some(reason) = event_loop.check_completion_event() {
            return Ok(LateEventRecovery::Terminate(reason));
        }

        if event_loop.has_pending_events() {
            return Ok(LateEventRecovery::PendingWork);
        }

        let observed_events = processed.had_events
            || processed.has_orphans
            || processed.human_interact_context.is_some();

        if observed_events {
            debug!(
                had_events = processed.had_events,
                has_orphans = processed.has_orphans,
                had_human_interact = processed.human_interact_context.is_some(),
                "Late JSONL drain found events but no new pending work"
            );
            return Ok(LateEventRecovery::NoLateEvents);
        }

        if poll_attempt == max_polls {
            break;
        }

        std::thread::sleep(Duration::from_millis(poll_interval_ms));
    }

    Ok(LateEventRecovery::NoLateEvents)
}

fn recover_late_events_before_fallback(
    event_loop: &mut EventLoop,
) -> std::io::Result<LateEventRecovery> {
    poll_for_late_events(
        event_loop,
        LATE_EVENT_RECOVERY_MAX_POLLS,
        LATE_EVENT_RECOVERY_POLL_INTERVAL_MS,
    )
}

fn recover_expected_emit_after_output(
    event_loop: &mut EventLoop,
) -> std::io::Result<LateEventRecovery> {
    poll_for_late_events(
        event_loop,
        EMIT_RECOVERY_MAX_POLLS,
        EMIT_RECOVERY_POLL_INTERVAL_MS,
    )
}

fn output_mentions_ralph_emit(output: &str) -> bool {
    output.contains("ralph emit")
}

fn resolve_display_hat_for_execution(
    event_loop: &EventLoop,
    hat_id: &HatId,
    preview_display_hat: &HatId,
) -> HatId {
    if hat_id.as_str() != "ralph" {
        return hat_id.clone();
    }

    event_loop
        .state()
        .last_active_hat_ids
        .first()
        .cloned()
        .unwrap_or_else(|| preview_display_hat.clone())
}

fn build_loop_start_payload_input(
    loop_id: &str,
    ctx: &LoopContext,
    max_iterations: u32,
    iteration_current: u32,
    active_hat: Option<String>,
    accumulated_metadata: &serde_json::Map<String, serde_json::Value>,
) -> HookPayloadBuilderInput {
    HookPayloadBuilderInput {
        loop_id: loop_id.to_string(),
        is_primary: ctx.is_primary(),
        workspace: ctx.workspace().to_path_buf(),
        repo_root: ctx.repo_root().to_path_buf(),
        pid: std::process::id(),
        iteration_current,
        iteration_max: max_iterations,
        context: HookPayloadContextInput {
            active_hat,
            metadata: accumulated_metadata.clone(),
            ..HookPayloadContextInput::default()
        },
    }
}

fn build_iteration_start_payload_input(
    loop_id: &str,
    ctx: &LoopContext,
    max_iterations: u32,
    iteration_current: u32,
    active_hat: Option<String>,
    selected_hat: Option<String>,
    selected_task: Option<String>,
    accumulated_metadata: &serde_json::Map<String, serde_json::Value>,
) -> HookPayloadBuilderInput {
    HookPayloadBuilderInput {
        loop_id: loop_id.to_string(),
        is_primary: ctx.is_primary(),
        workspace: ctx.workspace().to_path_buf(),
        repo_root: ctx.repo_root().to_path_buf(),
        pid: std::process::id(),
        iteration_current,
        iteration_max: max_iterations,
        context: HookPayloadContextInput {
            active_hat,
            selected_hat,
            selected_task,
            metadata: accumulated_metadata.clone(),
            ..HookPayloadContextInput::default()
        },
    }
}

fn build_plan_created_payload_input(
    loop_id: &str,
    ctx: &LoopContext,
    max_iterations: u32,
    iteration_current: u32,
    active_hat: Option<String>,
    selected_hat: Option<String>,
    selected_task: Option<String>,
    accumulated_metadata: &serde_json::Map<String, serde_json::Value>,
) -> HookPayloadBuilderInput {
    HookPayloadBuilderInput {
        loop_id: loop_id.to_string(),
        is_primary: ctx.is_primary(),
        workspace: ctx.workspace().to_path_buf(),
        repo_root: ctx.repo_root().to_path_buf(),
        pid: std::process::id(),
        iteration_current,
        iteration_max: max_iterations,
        context: HookPayloadContextInput {
            active_hat,
            selected_hat,
            selected_task,
            metadata: accumulated_metadata.clone(),
            ..HookPayloadContextInput::default()
        },
    }
}

fn build_human_interact_payload_input(
    loop_id: &str,
    ctx: &LoopContext,
    max_iterations: u32,
    iteration_current: u32,
    active_hat: Option<String>,
    selected_hat: Option<String>,
    selected_task: Option<String>,
    human_interact: Option<serde_json::Value>,
    accumulated_metadata: &serde_json::Map<String, serde_json::Value>,
) -> HookPayloadBuilderInput {
    HookPayloadBuilderInput {
        loop_id: loop_id.to_string(),
        is_primary: ctx.is_primary(),
        workspace: ctx.workspace().to_path_buf(),
        repo_root: ctx.repo_root().to_path_buf(),
        pid: std::process::id(),
        iteration_current,
        iteration_max: max_iterations,
        context: HookPayloadContextInput {
            active_hat,
            selected_hat,
            selected_task,
            human_interact,
            metadata: accumulated_metadata.clone(),
            ..HookPayloadContextInput::default()
        },
    }
}

fn build_loop_termination_payload_input(
    loop_id: &str,
    ctx: &LoopContext,
    max_iterations: u32,
    iteration_current: u32,
    active_hat: Option<String>,
    selected_hat: Option<String>,
    selected_task: Option<String>,
    termination_reason: &TerminationReason,
    accumulated_metadata: &serde_json::Map<String, serde_json::Value>,
) -> HookPayloadBuilderInput {
    HookPayloadBuilderInput {
        loop_id: loop_id.to_string(),
        is_primary: ctx.is_primary(),
        workspace: ctx.workspace().to_path_buf(),
        repo_root: ctx.repo_root().to_path_buf(),
        pid: std::process::id(),
        iteration_current,
        iteration_max: max_iterations,
        context: HookPayloadContextInput {
            active_hat,
            selected_hat,
            selected_task,
            termination_reason: Some(termination_reason.as_str().to_string()),
            metadata: accumulated_metadata.clone(),
            ..HookPayloadContextInput::default()
        },
    }
}

fn loop_termination_phase_events(reason: &TerminationReason) -> (HookPhaseEvent, HookPhaseEvent) {
    if reason.is_success() {
        (
            HookPhaseEvent::PreLoopComplete,
            HookPhaseEvent::PostLoopComplete,
        )
    } else {
        (HookPhaseEvent::PreLoopError, HookPhaseEvent::PostLoopError)
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_pre_loop_termination_hooks(
    event_loop: &EventLoop,
    hooks_dispatch_enabled: bool,
    loop_id: &str,
    hook_engine: &HookEngine,
    hook_executor: &HookExecutor,
    suspend_state_store: &SuspendStateStore,
    ctx: &LoopContext,
    max_iterations: u32,
    accumulated_hook_metadata: &mut serde_json::Map<String, serde_json::Value>,
    reason: TerminationReason,
) -> impl std::future::Future<Output = Result<TerminationReason>> + Send {
    let outcomes = collect_loop_termination_hook_outcomes(
        event_loop,
        hooks_dispatch_enabled,
        loop_id,
        hook_engine,
        hook_executor,
        ctx,
        max_iterations,
        accumulated_hook_metadata,
        &reason,
        true,
    );
    let loop_id = loop_id.to_string();
    let suspend_state_store = suspend_state_store.clone();

    async move {
        resolve_loop_termination_hook_outcomes(&outcomes, &loop_id, &suspend_state_store, reason)
            .await
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_post_loop_termination_hooks(
    event_loop: &EventLoop,
    hooks_dispatch_enabled: bool,
    loop_id: &str,
    hook_engine: &HookEngine,
    hook_executor: &HookExecutor,
    suspend_state_store: &SuspendStateStore,
    ctx: &LoopContext,
    max_iterations: u32,
    accumulated_hook_metadata: &mut serde_json::Map<String, serde_json::Value>,
    reason: TerminationReason,
) -> impl std::future::Future<Output = Result<TerminationReason>> + Send {
    let outcomes = collect_loop_termination_hook_outcomes(
        event_loop,
        hooks_dispatch_enabled,
        loop_id,
        hook_engine,
        hook_executor,
        ctx,
        max_iterations,
        accumulated_hook_metadata,
        &reason,
        false,
    );
    let loop_id = loop_id.to_string();
    let suspend_state_store = suspend_state_store.clone();

    async move {
        resolve_loop_termination_hook_outcomes(&outcomes, &loop_id, &suspend_state_store, reason)
            .await
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_loop_termination_hook_outcomes(
    event_loop: &EventLoop,
    hooks_dispatch_enabled: bool,
    loop_id: &str,
    hook_engine: &HookEngine,
    hook_executor: &HookExecutor,
    ctx: &LoopContext,
    max_iterations: u32,
    accumulated_hook_metadata: &mut serde_json::Map<String, serde_json::Value>,
    reason: &TerminationReason,
    is_pre_phase: bool,
) -> Vec<HookDispatchOutcome> {
    let (pre_phase_event, post_phase_event) = loop_termination_phase_events(reason);
    let phase_event = if is_pre_phase {
        pre_phase_event
    } else {
        post_phase_event
    };

    let active_hat = event_loop.get_active_hat_id().as_str().to_string();
    let outcomes = dispatch_phase_event_hooks(
        event_loop,
        hooks_dispatch_enabled,
        loop_id,
        hook_engine,
        hook_executor,
        phase_event,
        build_loop_termination_payload_input(
            loop_id,
            ctx,
            max_iterations,
            event_loop.state().iteration,
            Some(active_hat.clone()),
            Some(active_hat),
            None,
            reason,
            accumulated_hook_metadata,
        ),
    );
    merge_accumulated_hook_metadata_from_outcomes(accumulated_hook_metadata, &outcomes);
    outcomes
}

async fn resolve_loop_termination_hook_outcomes(
    outcomes: &[HookDispatchOutcome],
    loop_id: &str,
    suspend_state_store: &SuspendStateStore,
    reason: TerminationReason,
) -> Result<TerminationReason> {
    fail_if_blocking_loop_termination_outcomes(outcomes)?;

    if let Some(termination_reason) =
        wait_for_resume_if_suspended(outcomes, loop_id, suspend_state_store).await?
    {
        return Ok(termination_reason);
    }

    Ok(reason)
}

const RETRY_BACKOFF_DELAYS_MS: [u64; 3] = [100, 200, 400];
const RETRY_BACKOFF_SIGNAL_POLL_INTERVAL_MS: u64 = 100;
const SUSPEND_WAIT_SIGNAL_POLL_INTERVAL_MS: u64 = 250;
const HOOK_MUTATION_PAYLOAD_METADATA_KEY: &str = "metadata";
const HOOK_MUTATION_METADATA_NAMESPACE_KEY: &str = "hook_metadata";

#[derive(Debug, Clone, PartialEq)]
enum HookMutationParseOutcome {
    Disabled,
    Parsed {
        namespaced_metadata: serde_json::Map<String, serde_json::Value>,
    },
    Invalid(HookMutationParseError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HookMutationParseError {
    InvalidJson { message: String },
    InvalidSchema { message: String },
}

fn format_hook_mutation_parse_error(error: &HookMutationParseError) -> String {
    match error {
        HookMutationParseError::InvalidJson { message }
        | HookMutationParseError::InvalidSchema { message } => message.clone(),
    }
}

fn parse_hook_mutation_stdout(
    mutate: &HookMutationConfig,
    hook_name: &str,
    stdout: &str,
) -> HookMutationParseOutcome {
    if !mutate.enabled {
        return HookMutationParseOutcome::Disabled;
    }

    let parsed = match serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        Ok(parsed) => parsed,
        Err(error) => {
            return HookMutationParseOutcome::Invalid(HookMutationParseError::InvalidJson {
                message: format!("mutation stdout is not valid JSON: {error}"),
            });
        }
    };

    let Some(payload_object) = parsed.as_object() else {
        return HookMutationParseOutcome::Invalid(HookMutationParseError::InvalidSchema {
            message: "mutation payload must be a JSON object".to_string(),
        });
    };

    if payload_object.len() != 1 || !payload_object.contains_key(HOOK_MUTATION_PAYLOAD_METADATA_KEY)
    {
        let keys = payload_object.keys().cloned().collect::<Vec<_>>();
        return HookMutationParseOutcome::Invalid(HookMutationParseError::InvalidSchema {
            message: format!(
                "mutation payload supports only '{{\"{HOOK_MUTATION_PAYLOAD_METADATA_KEY}\": {{...}}}}'; found keys: {keys:?}"
            ),
        });
    }

    let Some(metadata) = payload_object
        .get(HOOK_MUTATION_PAYLOAD_METADATA_KEY)
        .and_then(serde_json::Value::as_object)
        .cloned()
    else {
        return HookMutationParseOutcome::Invalid(HookMutationParseError::InvalidSchema {
            message: "mutation payload key 'metadata' must contain a JSON object".to_string(),
        });
    };

    let mut namespaced_metadata = serde_json::Map::new();
    if let Err(error) = merge_hook_metadata_namespace(&mut namespaced_metadata, hook_name, metadata)
    {
        return HookMutationParseOutcome::Invalid(error);
    }

    HookMutationParseOutcome::Parsed {
        namespaced_metadata,
    }
}

fn merge_hook_metadata_namespace(
    accumulated_metadata: &mut serde_json::Map<String, serde_json::Value>,
    hook_name: &str,
    metadata: serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<(), HookMutationParseError> {
    if hook_name.trim().is_empty() {
        return Err(HookMutationParseError::InvalidSchema {
            message: "hook metadata namespace requires non-empty hook name".to_string(),
        });
    }

    let namespace = accumulated_metadata
        .entry(HOOK_MUTATION_METADATA_NAMESPACE_KEY.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

    let Some(namespace_object) = namespace.as_object_mut() else {
        return Err(HookMutationParseError::InvalidSchema {
            message: format!(
                "metadata namespace '{HOOK_MUTATION_METADATA_NAMESPACE_KEY}' must be a JSON object"
            ),
        });
    };

    namespace_object.insert(hook_name.to_string(), serde_json::Value::Object(metadata));
    Ok(())
}

fn merge_namespaced_hook_metadata(
    accumulated_metadata: &mut serde_json::Map<String, serde_json::Value>,
    namespaced_metadata: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<(), HookMutationParseError> {
    let Some(namespace_object) = namespaced_metadata
        .get(HOOK_MUTATION_METADATA_NAMESPACE_KEY)
        .and_then(serde_json::Value::as_object)
    else {
        return Err(HookMutationParseError::InvalidSchema {
            message: format!(
                "parsed mutation metadata must contain object key '{HOOK_MUTATION_METADATA_NAMESPACE_KEY}'"
            ),
        });
    };

    for (hook_name, metadata_value) in namespace_object {
        let Some(metadata_object) = metadata_value.as_object().cloned() else {
            return Err(HookMutationParseError::InvalidSchema {
                message: format!(
                    "parsed metadata entry for hook '{hook_name}' must be a JSON object"
                ),
            });
        };

        merge_hook_metadata_namespace(accumulated_metadata, hook_name, metadata_object)?;
    }

    Ok(())
}

fn merge_accumulated_hook_metadata_from_outcomes(
    accumulated_hook_metadata: &mut serde_json::Map<String, serde_json::Value>,
    outcomes: &[HookDispatchOutcome],
) {
    for outcome in outcomes {
        let HookMutationParseOutcome::Parsed {
            namespaced_metadata,
        } = &outcome.mutation_parse_outcome
        else {
            continue;
        };

        if let Err(error) =
            merge_namespaced_hook_metadata(accumulated_hook_metadata, namespaced_metadata)
        {
            warn!(
                phase_event = %outcome.phase_event,
                hook_name = %outcome.hook_name,
                error = ?error,
                "Failed to merge parsed hook mutation metadata; ignoring mutation output"
            );
        }
    }
}

fn mutation_parse_failure(
    mutation_parse_outcome: &HookMutationParseOutcome,
) -> Option<HookDispatchFailure> {
    let HookMutationParseOutcome::Invalid(error) = mutation_parse_outcome else {
        return None;
    };

    Some(HookDispatchFailure::InvalidMutationOutput {
        message: format_hook_mutation_parse_error(error),
    })
}

fn max_retry_attempts_for_suspend_mode(suspend_mode: HookSuspendMode) -> u32 {
    match suspend_mode {
        HookSuspendMode::WaitForResume => 1,
        HookSuspendMode::RetryBackoff => RETRY_BACKOFF_DELAYS_MS.len() as u32 + 1,
        HookSuspendMode::WaitThenRetry => 2,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuspendWaitOutcome {
    Resume,
    Stop,
    Restart,
}

#[derive(Debug, Clone, PartialEq)]
struct HookDispatchOutcome {
    phase_event: HookPhaseEvent,
    hook_name: String,
    disposition: HookDisposition,
    suspend_mode: HookSuspendMode,
    failure: Option<HookDispatchFailure>,
    mutation_parse_outcome: HookMutationParseOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HookDispatchFailure {
    HookRunFailed {
        exit_code: Option<i32>,
        timed_out: bool,
    },
    HookExecutionError {
        message: String,
    },
    InvalidMutationOutput {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetryBackoffDelayOutcome {
    Elapsed,
    StopRequested,
    RestartRequested,
}

fn dispatch_phase_event_hooks(
    event_loop: &EventLoop,
    hooks_enabled: bool,
    loop_id: &str,
    hook_engine: &HookEngine,
    hook_executor: &HookExecutor,
    phase_event: HookPhaseEvent,
    payload_input: HookPayloadBuilderInput,
) -> Vec<HookDispatchOutcome> {
    if !hooks_enabled {
        return Vec::new();
    }

    let resolved_hooks = hook_engine.resolve_phase_event(phase_event);
    if resolved_hooks.is_empty() {
        return Vec::new();
    }

    let workspace_root = payload_input.workspace.clone();
    let payload = hook_engine.build_payload(phase_event, payload_input);
    let stdin_payload = match serde_json::to_value(&payload) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                phase_event = %phase_event,
                error = %error,
                "Failed to serialize lifecycle hook payload; skipping phase-event dispatch"
            );
            return Vec::new();
        }
    };

    let mut outcomes = Vec::with_capacity(resolved_hooks.len());

    for hook in resolved_hooks {
        let hook_name = hook.name.clone();
        let phase_event_key = hook.phase_event.as_str().to_string();

        let request = HookRunRequest {
            phase_event: phase_event_key.clone(),
            hook_name: hook_name.clone(),
            command: hook.command.clone(),
            workspace_root: workspace_root.clone(),
            cwd: hook.cwd.clone(),
            env: hook.env.clone(),
            timeout_seconds: hook.timeout_seconds,
            max_output_bytes: hook.max_output_bytes,
            stdin_payload: stdin_payload.clone(),
        };

        let outcome = dispatch_hook_with_suspend_policy(
            event_loop,
            hook_executor,
            loop_id,
            &phase_event_key,
            hook.phase_event,
            &hook_name,
            hook.on_error,
            hook.suspend_mode,
            &hook.mutate,
            &request,
        );
        outcomes.push(outcome);
    }

    outcomes
}

#[allow(clippy::too_many_arguments)]
fn dispatch_hook_with_suspend_policy(
    event_loop: &EventLoop,
    hook_executor: &HookExecutor,
    loop_id: &str,
    phase_event_key: &str,
    phase_event: HookPhaseEvent,
    hook_name: &str,
    on_error: HookOnError,
    suspend_mode: HookSuspendMode,
    mutate: &HookMutationConfig,
    request: &HookRunRequest,
) -> HookDispatchOutcome {
    let retry_max_attempts = max_retry_attempts_for_suspend_mode(suspend_mode);
    let outcome = execute_hook_attempt(
        event_loop,
        hook_executor,
        loop_id,
        phase_event_key,
        phase_event,
        hook_name,
        on_error,
        suspend_mode,
        mutate,
        1,
        retry_max_attempts,
        request,
    );

    if outcome.disposition != HookDisposition::Suspend {
        return outcome;
    }

    match suspend_mode {
        HookSuspendMode::WaitForResume => outcome,
        HookSuspendMode::RetryBackoff => dispatch_retry_backoff_suspend_policy(
            event_loop,
            hook_executor,
            loop_id,
            phase_event_key,
            phase_event,
            hook_name,
            on_error,
            suspend_mode,
            mutate,
            retry_max_attempts,
            request,
            outcome,
        ),
        HookSuspendMode::WaitThenRetry => dispatch_wait_then_retry_suspend_policy(
            event_loop,
            hook_executor,
            loop_id,
            phase_event_key,
            phase_event,
            hook_name,
            on_error,
            suspend_mode,
            mutate,
            retry_max_attempts,
            request,
            outcome,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_retry_backoff_suspend_policy(
    event_loop: &EventLoop,
    hook_executor: &HookExecutor,
    loop_id: &str,
    phase_event_key: &str,
    phase_event: HookPhaseEvent,
    hook_name: &str,
    on_error: HookOnError,
    suspend_mode: HookSuspendMode,
    mutate: &HookMutationConfig,
    retry_max_attempts: u32,
    request: &HookRunRequest,
    outcome: HookDispatchOutcome,
) -> HookDispatchOutcome {
    run_retry_backoff_policy(
        phase_event_key,
        hook_name,
        &RETRY_BACKOFF_DELAYS_MS,
        |backoff_delay, _retry_attempt| {
            wait_for_retry_backoff_delay_with_signal_poll(
                request.workspace_root.as_path(),
                backoff_delay,
            )
        },
        |retry_attempt| {
            execute_hook_attempt(
                event_loop,
                hook_executor,
                loop_id,
                phase_event_key,
                phase_event,
                hook_name,
                on_error,
                suspend_mode,
                mutate,
                retry_attempt,
                retry_max_attempts,
                request,
            )
        },
        outcome,
    )
}

#[allow(clippy::too_many_arguments)]
fn dispatch_wait_then_retry_suspend_policy(
    event_loop: &EventLoop,
    hook_executor: &HookExecutor,
    loop_id: &str,
    phase_event_key: &str,
    phase_event: HookPhaseEvent,
    hook_name: &str,
    on_error: HookOnError,
    suspend_mode: HookSuspendMode,
    mutate: &HookMutationConfig,
    retry_max_attempts: u32,
    request: &HookRunRequest,
    outcome: HookDispatchOutcome,
) -> HookDispatchOutcome {
    let suspend_state_store = SuspendStateStore::new(&request.workspace_root);
    let reason = format_suspending_hook_reason(&outcome);
    let suspend_state = SuspendStateRecord::new(
        loop_id,
        phase_event,
        hook_name,
        reason,
        suspend_mode,
        chrono::Utc::now(),
    );

    if let Err(error) = suspend_state_store.write_suspend_state(&suspend_state) {
        warn!(
            phase_event = %phase_event_key,
            hook_name = %hook_name,
            error = %error,
            "Failed to persist suspend-state for wait_then_retry; deferring to standard suspend handling"
        );
        return outcome;
    }

    warn!(
        phase_event = %phase_event_key,
        hook_name = %hook_name,
        "Lifecycle hook requested suspend(wait_then_retry); entering wait-for-resume gate before single retry"
    );

    run_wait_then_retry_policy(
        phase_event_key,
        hook_name,
        || wait_for_suspend_signal_with_poll(&suspend_state_store),
        || {
            suspend_state_store
                .clear_suspend_state()
                .context("Failed to clear wait_then_retry suspend-state after resume")?;
            Ok(())
        },
        || {
            execute_hook_attempt(
                event_loop,
                hook_executor,
                loop_id,
                phase_event_key,
                phase_event,
                hook_name,
                on_error,
                suspend_mode,
                mutate,
                2,
                retry_max_attempts,
                request,
            )
        },
        outcome,
    )
}

fn run_retry_backoff_policy<FWaitForDelay, FRunRetryAttempt>(
    phase_event_key: &str,
    hook_name: &str,
    backoff_delays_ms: &[u64],
    mut wait_for_delay: FWaitForDelay,
    mut run_retry_attempt: FRunRetryAttempt,
    mut outcome: HookDispatchOutcome,
) -> HookDispatchOutcome
where
    FWaitForDelay: FnMut(Duration, usize) -> RetryBackoffDelayOutcome,
    FRunRetryAttempt: FnMut(u32) -> HookDispatchOutcome,
{
    for (retry_attempt, backoff_delay_ms) in backoff_delays_ms.iter().copied().enumerate() {
        match wait_for_delay(Duration::from_millis(backoff_delay_ms), retry_attempt + 1) {
            RetryBackoffDelayOutcome::Elapsed => {}
            RetryBackoffDelayOutcome::StopRequested => {
                info!(
                    phase_event = %phase_event_key,
                    hook_name = %hook_name,
                    retry_attempt = retry_attempt + 1,
                    "Stop requested while waiting for retry_backoff retry; deferring to suspend termination handling"
                );
                break;
            }
            RetryBackoffDelayOutcome::RestartRequested => {
                info!(
                    phase_event = %phase_event_key,
                    hook_name = %hook_name,
                    retry_attempt = retry_attempt + 1,
                    "Restart requested while waiting for retry_backoff retry; deferring to suspend termination handling"
                );
                break;
            }
        }

        outcome = run_retry_attempt(retry_attempt as u32 + 2);

        if outcome.disposition == HookDisposition::Pass {
            info!(
                phase_event = %phase_event_key,
                hook_name = %hook_name,
                retry_attempt = retry_attempt + 1,
                "Lifecycle hook recovered under retry_backoff"
            );
            return outcome;
        }

        if outcome.disposition != HookDisposition::Suspend {
            return outcome;
        }
    }

    warn!(
        phase_event = %phase_event_key,
        hook_name = %hook_name,
        retry_attempts = backoff_delays_ms.len(),
        "Lifecycle hook retry_backoff policy exhausted; entering suspended wait_for_resume fallback"
    );

    outcome
}

fn run_wait_then_retry_policy<FWaitForSignal, FClearSuspendState, FRunRetryAttempt>(
    phase_event_key: &str,
    hook_name: &str,
    mut wait_for_signal: FWaitForSignal,
    mut clear_suspend_state: FClearSuspendState,
    mut run_retry_attempt: FRunRetryAttempt,
    outcome: HookDispatchOutcome,
) -> HookDispatchOutcome
where
    FWaitForSignal: FnMut() -> Result<SuspendWaitOutcome>,
    FClearSuspendState: FnMut() -> Result<()>,
    FRunRetryAttempt: FnMut() -> HookDispatchOutcome,
{
    let wait_outcome = match wait_for_signal() {
        Ok(wait_outcome) => wait_outcome,
        Err(error) => {
            warn!(
                phase_event = %phase_event_key,
                hook_name = %hook_name,
                error = %error,
                "wait_then_retry gate failed while polling suspend signals; deferring to standard suspend handling"
            );
            return outcome;
        }
    };

    match wait_outcome {
        SuspendWaitOutcome::Stop => {
            info!(
                phase_event = %phase_event_key,
                hook_name = %hook_name,
                "Stop requested while waiting under wait_then_retry; deferring to suspend termination handling"
            );
            outcome
        }
        SuspendWaitOutcome::Restart => {
            info!(
                phase_event = %phase_event_key,
                hook_name = %hook_name,
                "Restart requested while waiting under wait_then_retry; deferring to suspend termination handling"
            );
            outcome
        }
        SuspendWaitOutcome::Resume => {
            if let Err(error) = clear_suspend_state() {
                warn!(
                    phase_event = %phase_event_key,
                    hook_name = %hook_name,
                    error = %error,
                    "Failed to clear wait_then_retry suspend-state after resume; deferring to standard suspend handling"
                );
                return outcome;
            }

            let retry_outcome = run_retry_attempt();

            if retry_outcome.disposition == HookDisposition::Pass {
                info!(
                    phase_event = %phase_event_key,
                    hook_name = %hook_name,
                    "Lifecycle hook recovered under wait_then_retry"
                );
            }

            retry_outcome
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_hook_attempt(
    event_loop: &EventLoop,
    hook_executor: &HookExecutor,
    loop_id: &str,
    phase_event_key: &str,
    phase_event: HookPhaseEvent,
    hook_name: &str,
    on_error: HookOnError,
    suspend_mode: HookSuspendMode,
    mutate: &HookMutationConfig,
    retry_attempt: u32,
    retry_max_attempts: u32,
    request: &HookRunRequest,
) -> HookDispatchOutcome {
    match hook_executor.run(request.clone()) {
        Ok(run_result) => {
            let run_disposition = classify_hook_disposition(on_error, &run_result);
            let mutation_parse_outcome =
                parse_hook_mutation_stdout(mutate, hook_name, &run_result.stdout.content);
            let mutation_failure = if run_disposition == HookDisposition::Pass {
                mutation_parse_failure(&mutation_parse_outcome)
            } else {
                None
            };

            let disposition = if mutation_failure.is_some() {
                disposition_from_on_error(on_error)
            } else {
                run_disposition
            };

            let failure = if let Some(mutation_failure) = mutation_failure {
                Some(mutation_failure)
            } else if run_disposition == HookDisposition::Pass {
                None
            } else {
                Some(HookDispatchFailure::HookRunFailed {
                    exit_code: run_result.exit_code,
                    timed_out: run_result.timed_out,
                })
            };

            event_loop.log_hook_run_telemetry(HookRunTelemetryEntry::from_run_result(
                loop_id,
                phase_event_key,
                hook_name,
                disposition,
                suspend_mode,
                retry_attempt,
                retry_max_attempts,
                &run_result,
            ));

            if disposition == HookDisposition::Pass {
                debug!(
                    phase_event = %phase_event_key,
                    hook_name = %hook_name,
                    duration_ms = run_result.duration_ms,
                    "Lifecycle hook executed successfully"
                );
            } else {
                let failure_detail = format_hook_failure_detail(failure.as_ref());
                warn!(
                    phase_event = %phase_event_key,
                    hook_name = %hook_name,
                    disposition = ?disposition,
                    exit_code = ?run_result.exit_code,
                    timed_out = run_result.timed_out,
                    failure = %failure_detail,
                    "Lifecycle hook returned non-pass disposition; continuing"
                );
            }

            HookDispatchOutcome {
                phase_event,
                hook_name: hook_name.to_string(),
                disposition,
                suspend_mode,
                failure,
                mutation_parse_outcome,
            }
        }
        Err(error) => {
            let disposition = disposition_from_on_error(on_error);

            warn!(
                phase_event = %phase_event_key,
                hook_name = %hook_name,
                disposition = ?disposition,
                error = %error,
                "Lifecycle hook execution failed; continuing"
            );

            HookDispatchOutcome {
                phase_event,
                hook_name: hook_name.to_string(),
                disposition,
                suspend_mode,
                failure: Some(HookDispatchFailure::HookExecutionError {
                    message: error.to_string(),
                }),
                mutation_parse_outcome: HookMutationParseOutcome::Disabled,
            }
        }
    }
}

fn wait_for_retry_backoff_delay_with_signal_poll(
    workspace_root: &Path,
    backoff_delay: Duration,
) -> RetryBackoffDelayOutcome {
    if backoff_delay.is_zero() {
        return RetryBackoffDelayOutcome::Elapsed;
    }

    let poll_interval = Duration::from_millis(RETRY_BACKOFF_SIGNAL_POLL_INTERVAL_MS);
    let sleep_started_at = std::time::Instant::now();

    loop {
        if is_stop_requested(workspace_root) {
            return RetryBackoffDelayOutcome::StopRequested;
        }

        if is_restart_requested(workspace_root) {
            return RetryBackoffDelayOutcome::RestartRequested;
        }

        let elapsed = sleep_started_at.elapsed();
        if elapsed >= backoff_delay {
            return RetryBackoffDelayOutcome::Elapsed;
        }

        let remaining = backoff_delay.saturating_sub(elapsed);
        std::thread::sleep(std::cmp::min(remaining, poll_interval));
    }
}

fn wait_for_suspend_signal_with_poll(
    suspend_state_store: &SuspendStateStore,
) -> Result<SuspendWaitOutcome> {
    let poll_interval = Duration::from_millis(SUSPEND_WAIT_SIGNAL_POLL_INTERVAL_MS);

    loop {
        if is_stop_requested(suspend_state_store.workspace_root()) {
            return Ok(SuspendWaitOutcome::Stop);
        }

        if is_restart_requested(suspend_state_store.workspace_root()) {
            return Ok(SuspendWaitOutcome::Restart);
        }

        if suspend_state_store
            .consume_resume_requested()
            .context("Failed to consume resume signal while suspended")?
        {
            return Ok(SuspendWaitOutcome::Resume);
        }

        std::thread::sleep(poll_interval);
    }
}

fn fail_if_blocking_loop_start_outcomes(outcomes: &[HookDispatchOutcome]) -> Result<()> {
    let Some(blocking_outcome) = outcomes
        .iter()
        .find(|outcome| outcome.disposition == HookDisposition::Block)
    else {
        return Ok(());
    };

    let reason = format_blocking_hook_reason(blocking_outcome);
    error!(
        phase_event = %blocking_outcome.phase_event,
        hook_name = %blocking_outcome.hook_name,
        reason = %reason,
        "Lifecycle hook blocked loop.start boundary"
    );

    Err(anyhow::anyhow!(reason))
}

fn fail_if_blocking_iteration_start_outcomes(outcomes: &[HookDispatchOutcome]) -> Result<()> {
    let Some(blocking_outcome) = outcomes
        .iter()
        .find(|outcome| outcome.disposition == HookDisposition::Block)
    else {
        return Ok(());
    };

    let reason = format_blocking_hook_reason(blocking_outcome);
    error!(
        phase_event = %blocking_outcome.phase_event,
        hook_name = %blocking_outcome.hook_name,
        reason = %reason,
        "Lifecycle hook blocked iteration.start boundary"
    );

    Err(anyhow::anyhow!(reason))
}

fn fail_if_blocking_plan_created_outcomes(outcomes: &[HookDispatchOutcome]) -> Result<()> {
    let Some(blocking_outcome) = outcomes
        .iter()
        .find(|outcome| outcome.disposition == HookDisposition::Block)
    else {
        return Ok(());
    };

    let reason = format_blocking_hook_reason(blocking_outcome);
    error!(
        phase_event = %blocking_outcome.phase_event,
        hook_name = %blocking_outcome.hook_name,
        reason = %reason,
        "Lifecycle hook blocked plan.created boundary"
    );

    Err(anyhow::anyhow!(reason))
}

fn fail_if_blocking_human_interact_outcomes(outcomes: &[HookDispatchOutcome]) -> Result<()> {
    let Some(blocking_outcome) = outcomes
        .iter()
        .find(|outcome| outcome.disposition == HookDisposition::Block)
    else {
        return Ok(());
    };

    let reason = format_blocking_hook_reason(blocking_outcome);
    error!(
        phase_event = %blocking_outcome.phase_event,
        hook_name = %blocking_outcome.hook_name,
        reason = %reason,
        "Lifecycle hook blocked human.interact boundary"
    );

    Err(anyhow::anyhow!(reason))
}

fn fail_if_blocking_loop_termination_outcomes(outcomes: &[HookDispatchOutcome]) -> Result<()> {
    let Some(blocking_outcome) = outcomes
        .iter()
        .find(|outcome| outcome.disposition == HookDisposition::Block)
    else {
        return Ok(());
    };

    let reason = format_blocking_hook_reason(blocking_outcome);
    error!(
        phase_event = %blocking_outcome.phase_event,
        hook_name = %blocking_outcome.hook_name,
        reason = %reason,
        "Lifecycle hook blocked loop termination boundary"
    );

    Err(anyhow::anyhow!(reason))
}

async fn wait_for_resume_if_suspended(
    outcomes: &[HookDispatchOutcome],
    loop_id: &str,
    suspend_state_store: &SuspendStateStore,
) -> Result<Option<TerminationReason>> {
    let Some(suspending_outcome) = outcomes
        .iter()
        .find(|outcome| outcome.disposition == HookDisposition::Suspend)
    else {
        return Ok(None);
    };

    let reason = format_suspending_hook_reason(suspending_outcome);
    let suspend_state = SuspendStateRecord::new(
        loop_id,
        suspending_outcome.phase_event,
        &suspending_outcome.hook_name,
        &reason,
        suspending_outcome.suspend_mode,
        chrono::Utc::now(),
    );

    suspend_state_store
        .write_suspend_state(&suspend_state)
        .with_context(|| {
            format!(
                "Failed to persist suspend-state for hook '{}' at '{}'",
                suspending_outcome.hook_name,
                suspending_outcome.phase_event.as_str()
            )
        })?;

    warn!(
        phase_event = %suspending_outcome.phase_event,
        hook_name = %suspending_outcome.hook_name,
        suspend_mode = ?suspending_outcome.suspend_mode,
        reason = %reason,
        "Lifecycle hook requested suspend; entering wait_for_resume gate"
    );

    loop {
        if consume_stop_requested_signal(suspend_state_store.workspace_root())? {
            clear_suspend_wait_artifacts(suspend_state_store)?;
            info!(
                phase_event = %suspending_outcome.phase_event,
                hook_name = %suspending_outcome.hook_name,
                "Stop requested while suspended; terminating loop"
            );
            return Ok(Some(TerminationReason::Stopped));
        }

        if is_restart_requested(suspend_state_store.workspace_root()) {
            clear_suspend_wait_artifacts(suspend_state_store)?;
            info!(
                phase_event = %suspending_outcome.phase_event,
                hook_name = %suspending_outcome.hook_name,
                "Restart requested while suspended; terminating loop for restart"
            );
            return Ok(Some(TerminationReason::RestartRequested));
        }

        if suspend_state_store
            .consume_resume_requested()
            .context("Failed to consume resume signal while suspended")?
        {
            suspend_state_store
                .clear_suspend_state()
                .context("Failed to clear suspend-state after resume signal")?;

            info!(
                phase_event = %suspending_outcome.phase_event,
                hook_name = %suspending_outcome.hook_name,
                "Resume signal consumed; leaving suspended wait_for_resume state"
            );
            return Ok(None);
        }

        tokio::time::sleep(Duration::from_millis(SUSPEND_WAIT_SIGNAL_POLL_INTERVAL_MS)).await;
    }
}

fn clear_suspend_wait_artifacts(suspend_state_store: &SuspendStateStore) -> Result<()> {
    suspend_state_store
        .clear_suspend_state()
        .context("Failed to clear suspend-state artifact")?;
    suspend_state_store
        .consume_resume_requested()
        .context("Failed to clear stale resume signal")?;
    Ok(())
}

fn is_stop_requested(workspace_root: &Path) -> bool {
    workspace_root.join(".ralph/stop-requested").exists()
}

fn consume_stop_requested_signal(workspace_root: &Path) -> Result<bool> {
    let stop_path = workspace_root.join(".ralph/stop-requested");
    match fs::remove_file(&stop_path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(anyhow::Error::new(error)).with_context(|| {
            format!(
                "Failed to consume stop signal while suspended: {}",
                stop_path.display()
            )
        }),
    }
}

fn is_restart_requested(workspace_root: &Path) -> bool {
    workspace_root.join(".ralph/restart-requested").exists()
}

fn format_suspending_hook_reason(outcome: &HookDispatchOutcome) -> String {
    format!(
        "Lifecycle hook '{}' suspended orchestration at '{}': {}",
        outcome.hook_name,
        outcome.phase_event.as_str(),
        format_hook_failure_detail(outcome.failure.as_ref())
    )
}

fn format_blocking_hook_reason(outcome: &HookDispatchOutcome) -> String {
    format!(
        "Lifecycle hook '{}' blocked orchestration at '{}': {}",
        outcome.hook_name,
        outcome.phase_event.as_str(),
        format_hook_failure_detail(outcome.failure.as_ref())
    )
}

fn format_hook_failure_detail(failure: Option<&HookDispatchFailure>) -> String {
    match failure {
        Some(HookDispatchFailure::HookRunFailed {
            exit_code,
            timed_out,
        }) => {
            if *timed_out {
                "hook timed out".to_string()
            } else if let Some(code) = exit_code {
                format!("hook exited with code {code}")
            } else {
                "hook exited unsuccessfully".to_string()
            }
        }
        Some(HookDispatchFailure::HookExecutionError { message }) => {
            format!("hook execution failed: {message}")
        }
        Some(HookDispatchFailure::InvalidMutationOutput { message }) => {
            format!("invalid mutation output: {message}")
        }
        None => "hook failed without failure details".to_string(),
    }
}

fn classify_hook_disposition(on_error: HookOnError, run_result: &HookRunResult) -> HookDisposition {
    if !run_result.timed_out && run_result.exit_code == Some(0) {
        HookDisposition::Pass
    } else {
        disposition_from_on_error(on_error)
    }
}

fn disposition_from_on_error(on_error: HookOnError) -> HookDisposition {
    match on_error {
        HookOnError::Warn => HookDisposition::Warn,
        HookOnError::Block => HookDisposition::Block,
        HookOnError::Suspend => HookDisposition::Suspend,
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

fn detect_solo_output_completion(
    registry: &HatRegistry,
    output: &str,
    completion_promise: &str,
) -> bool {
    registry.is_empty() && EventParser::contains_promise(output, completion_promise)
}

fn normalize_cli_output_for_parsing(
    output_format: BackendOutputFormat,
    raw_output: &str,
) -> String {
    match output_format {
        BackendOutputFormat::StreamJson => extract_claude_stream_text(raw_output),
        BackendOutputFormat::PiStreamJson => extract_pi_stream_text(raw_output),
        _ => raw_output.to_string(),
    }
}

fn extract_claude_stream_text(raw_output: &str) -> String {
    let mut extracted = String::new();

    for line in raw_output.lines() {
        let Some(event) = ClaudeStreamParser::parse_line(line) else {
            continue;
        };

        if let ClaudeStreamEvent::Assistant { message, .. } = event {
            for block in message.content {
                if let ContentBlock::Text { text } = block {
                    extracted.push_str(&text);
                    extracted.push('\n');
                }
            }
        }
    }

    if extracted.is_empty() {
        raw_output.to_string()
    } else {
        extracted
    }
}

fn extract_pi_stream_text(raw_output: &str) -> String {
    let mut extracted = String::new();

    for line in raw_output.lines() {
        let Some(event) = PiStreamParser::parse_line(line) else {
            continue;
        };

        if let PiStreamEvent::MessageUpdate {
            assistant_message_event,
        } = event
            && let PiAssistantEvent::TextDelta { delta } = assistant_message_event
        {
            extracted.push_str(&delta);
        }
    }

    if extracted.is_empty() {
        raw_output.to_string()
    } else {
        extracted
    }
}

/// Resolves the active timestamped events JSONL file path for this run.
///
/// The authoritative source is `.ralph/current-events`, which contains a
/// relative path like `.ralph/events-YYYYMMDD-HHMMSS.jsonl`.
///
/// Falls back to `ctx.events_path()` if the marker is missing/unreadable.
fn resolve_current_events_path(ctx: &LoopContext) -> PathBuf {
    fs::read_to_string(ctx.current_events_marker())
        .ok()
        .map(|relative| {
            let relative = relative.trim().to_string();
            if std::path::Path::new(&relative).is_relative() {
                ctx.workspace().join(relative)
            } else {
                PathBuf::from(relative)
            }
        })
        .unwrap_or_else(|| ctx.events_path())
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

/// Execute a prompt via ACP (Agent Client Protocol) for kiro-acp backend.
async fn execute_acp(
    backend: &CliBackend,
    config: &RalphConfig,
    prompt: &str,
    verbosity: Verbosity,
    tui_lines: Option<Arc<std::sync::Mutex<Vec<ratatui::text::Line<'static>>>>>,
    rpc_stdout: Option<Arc<std::sync::Mutex<std::io::Stdout>>>,
    iteration: u32,
    hat: &str,
    backend_name: &str,
) -> Result<ExecutionOutcome> {
    let executor = AcpExecutor::new(backend.clone(), config.core.workspace_root.clone());

    let pty_result = if let Some(lines) = tui_lines {
        let mut handler = TuiStreamHandler::with_lines(verbosity == Verbosity::Verbose, lines);
        executor.execute(prompt, &mut handler).await?
    } else if let Some(stdout_writer) = rpc_stdout {
        let mut handler = JsonRpcStreamHandler::new(
            stdout_writer,
            iteration,
            Some(hat.to_string()),
            Some(backend_name.to_string()),
        );
        executor.execute(prompt, &mut handler).await?
    } else {
        match verbosity {
            Verbosity::Quiet => {
                let mut handler = QuietStreamHandler;
                executor.execute(prompt, &mut handler).await?
            }
            Verbosity::Normal => {
                let mut handler = ConsoleStreamHandler::new(false);
                executor.execute(prompt, &mut handler).await?
            }
            Verbosity::Verbose => {
                let mut handler = ConsoleStreamHandler::new(true);
                executor.execute(prompt, &mut handler).await?
            }
        }
    };

    let output = if pty_result.extracted_text.is_empty() {
        pty_result.stripped_output
    } else {
        pty_result.extracted_text
    };

    Ok(ExecutionOutcome {
        output,
        success: pty_result.success,
        termination: None,
        total_cost_usd: pty_result.total_cost_usd,
        input_tokens: pty_result.input_tokens,
        output_tokens: pty_result.output_tokens,
        cache_read_tokens: pty_result.cache_read_tokens,
        cache_write_tokens: pty_result.cache_write_tokens,
    })
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
    rpc_stdout: Option<Arc<std::sync::Mutex<std::io::Stdout>>>,
    iteration: u32,
    hat: &str,
    backend_name: &str,
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
    let result = if interactive && tui_lines.is_none() && rpc_stdout.is_none() {
        // Raw interactive mode only when not using TUI or RPC (TUI/RPC handle their own I/O)
        exec.run_interactive(prompt, interrupt_rx).await
    } else if let Some(lines) = tui_lines {
        // TUI mode: use TuiStreamHandler to capture output for TUI display
        let verbose = verbosity == Verbosity::Verbose;
        let mut handler = TuiStreamHandler::with_lines(verbose, lines);
        exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
            .await
    } else if let Some(stdout_writer) = rpc_stdout {
        // RPC mode: use JsonRpcStreamHandler for JSON-lines output
        let mut handler = JsonRpcStreamHandler::new(
            stdout_writer,
            iteration,
            Some(hat.to_string()),
            Some(backend_name.to_string()),
        );
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
                total_cost_usd: pty_result.total_cost_usd,
                input_tokens: pty_result.input_tokens,
                output_tokens: pty_result.output_tokens,
                cache_read_tokens: pty_result.cache_read_tokens,
                cache_write_tokens: pty_result.cache_write_tokens,
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

    // Write a core-only merge config once (shared by all merge loops).
    let mut core_value: serde_yaml::Value = match serde_yaml::from_str(preset.content) {
        Ok(value) => value,
        Err(e) => {
            warn!(
                error = %e,
                "Failed to parse merge-loop preset, pending merges will remain queued"
            );
            return;
        }
    };

    if let Some(mapping) = core_value.as_mapping_mut() {
        let hats_key = serde_yaml::Value::String("hats".to_string());
        let events_key = serde_yaml::Value::String("events".to_string());
        mapping.remove(&hats_key);
        mapping.remove(&events_key);
    }

    let core_yaml = match serde_yaml::to_string(&core_value) {
        Ok(yaml) => yaml,
        Err(e) => {
            warn!(
                error = %e,
                "Failed to serialize core-only merge config, pending merges will remain queued"
            );
            return;
        }
    };

    let config_path = repo_root.join(".ralph/merge-loop-config.yml");
    if let Err(e) = fs::write(&config_path, core_yaml) {
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

        // Redirect subprocess stdio to a log file to prevent TUI corruption.
        // If log file creation fails, fall back to Stdio::null rather than
        // inheriting the parent's terminal (which would corrupt the TUI).
        let (stdout_stdio, stderr_stdio, log_path) =
            match create_merge_subprocess_log_file(repo_root, loop_id) {
                Ok((file, path)) => match file.try_clone() {
                    Ok(file_clone) => (Stdio::from(file_clone), Stdio::from(file), Some(path)),
                    Err(e) => {
                        warn!(
                            loop_id = %loop_id,
                            error = %e,
                            "Failed to clone log file handle, subprocess output will be discarded"
                        );
                        (Stdio::null(), Stdio::null(), None)
                    }
                },
                Err(e) => {
                    warn!(
                        loop_id = %loop_id,
                        error = %e,
                        "Failed to create subprocess log file, output will be discarded"
                    );
                    (Stdio::null(), Stdio::null(), None)
                }
            };

        match Command::new(ralph_cmd)
            .current_dir(repo_root)
            .args([
                "run",
                "-c",
                ".ralph/merge-loop-config.yml",
                "-H",
                "builtin:merge-loop",
                "--exclusive",
                "--no-tui",
                "-p",
                &format!("Merge loop {} from branch ralph/{}", loop_id, loop_id),
            ])
            .env("RALPH_MERGE_LOOP_ID", loop_id)
            .stdout(stdout_stdio)
            .stderr(stderr_stdio)
            .spawn()
        {
            Ok(child) => {
                if let Some(path) = log_path {
                    info!(
                        loop_id = %loop_id,
                        pid = child.id(),
                        log_file = %path.display(),
                        "merge-ralph spawned successfully"
                    );
                } else {
                    info!(
                        loop_id = %loop_id,
                        pid = child.id(),
                        "merge-ralph spawned successfully"
                    );
                }
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

/// Creates a timestamped log file for a merge subprocess under `.ralph/diagnostics/logs/`.
///
/// Uses the loop_id in the filename for easier identification when debugging.
/// Participates in the existing log rotation scheme.
fn create_merge_subprocess_log_file(
    repo_root: &Path,
    loop_id: &str,
) -> std::io::Result<(File, PathBuf)> {
    use chrono::Local;

    let logs_dir = repo_root.join(".ralph").join("diagnostics").join("logs");
    fs::create_dir_all(&logs_dir)?;

    let _ = ralph_core::diagnostics::rotate_logs(&logs_dir, 10);

    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S");
    let log_path = logs_dir.join(format!("ralph-merge-{}-{}.log", loop_id, timestamp));
    let file = File::create(&log_path)?;

    Ok((file, log_path))
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
        false, // no RPC
        Verbosity::Normal,
        None,               // no session recording
        Some(loop_context), // loop context
        Vec::new(),         // no custom args
        None,               // default auto-merge
        None,               // no explicit loop ID
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
    let api_url = config.robot.resolve_api_url();
    let timeout_secs = config.robot.timeout_seconds.unwrap_or(300);
    let loop_id = context
        .loop_id()
        .map(String::from)
        .unwrap_or_else(|| "main".to_string());

    match ralph_telegram::TelegramService::new(
        workspace_root,
        bot_token,
        api_url,
        timeout_secs,
        loop_id,
    ) {
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

// ── Wave Execution ──────────────────────────────────────────────────────────

/// Push a styled line to a TUI wave worker's output buffer.
fn push_to_wave_worker_buffer(
    state: &Arc<std::sync::Mutex<ralph_tui::TuiState>>,
    worker_idx: usize,
    lines: &[Line<'static>],
) {
    let Ok(s) = state.lock() else { return };
    let Some(ref wave) = s.wave_active else {
        return;
    };
    let Some(buffer) = wave.worker_buffers.get(worker_idx) else {
        return;
    };
    let handle = buffer.lines_handle();
    let Ok(mut buf_lines) = handle.lock() else {
        return;
    };
    buf_lines.extend_from_slice(lines);
}

/// Push a line to the latest iteration's output in the TUI.
fn push_to_tui_iteration(state: &Arc<std::sync::Mutex<ralph_tui::TuiState>>, line: Line<'static>) {
    let Ok(s) = state.lock() else { return };
    let Some(handle) = s.latest_iteration_lines_handle() else {
        return;
    };
    let Ok(mut lines) = handle.lock() else { return };
    lines.push(line);
}

/// Bundled output channels for wave progress reporting.
struct WaveOutputs<'a> {
    use_colors: bool,
    /// Show wave progress on CLI (no TUI, no RPC).
    show_cli: bool,
    rpc_tx: Option<&'a tokio::sync::mpsc::Sender<RpcEvent>>,
    tui: Option<&'a Arc<std::sync::Mutex<ralph_tui::TuiState>>>,
}

/// Handle wave events: detect, execute, merge results, and update UI.
///
/// Orchestrates the full wave lifecycle — detection, parallel execution,
/// result merging back to the events file, and re-reading for aggregator
/// activation. Updates CLI, RPC, and TUI outputs as appropriate.
async fn handle_wave_events(
    wave_events: &[ralph_core::Event],
    event_loop: &mut ralph_core::EventLoop,
    backend: &CliBackend,
    ctx: &LoopContext,
    use_colors: bool,
    enable_rpc: bool,
    rpc_event_tx: Option<&tokio::sync::mpsc::Sender<RpcEvent>>,
    tui_state: Option<&Arc<std::sync::Mutex<ralph_tui::TuiState>>>,
) {
    let Some(detected) = ralph_core::detect_wave_events(wave_events, event_loop.registry()) else {
        return;
    };

    info!(
        wave_id = %detected.wave_id,
        total = detected.total,
        hat = %detected.target_hat,
        concurrency = detected.hat_config.concurrency,
        "Wave detected, executing parallel workers"
    );

    let out = WaveOutputs {
        use_colors,
        show_cli: tui_state.is_none() && !enable_rpc,
        rpc_tx: rpc_event_tx,
        tui: tui_state,
    };

    let wave_timeout_secs = detected.timeout_secs();

    // Announce wave start to CLI / RPC / TUI
    if out.show_cli {
        print_wave_header(
            &detected.hat_config.name,
            detected.total as usize,
            wave_timeout_secs,
            out.use_colors,
        );
    }
    if let Some(tx) = out.rpc_tx {
        let _ = tx.try_send(RpcEvent::WaveStarted {
            hat_name: detected.hat_config.name.clone(),
            worker_count: detected.total,
            timeout_secs: wave_timeout_secs,
        });
    }
    if let Some(state) = out.tui {
        if let Ok(mut s) = state.lock() {
            info!(
                hat = %detected.hat_config.name,
                workers = detected.total,
                "Setting wave_active on TUI state"
            );
            s.wave_active = Some(ralph_tui::state::WaveInfo::new(
                detected.hat_config.name.clone(),
                detected.total,
            ));
            s.wave_active_iteration_idx = Some(s.iterations.len().saturating_sub(1));
            if let Some(ref wave) = s.wave_active {
                for (i, buffer) in wave.worker_buffers.iter().enumerate() {
                    if let Ok(mut buf_lines) = buffer.lines_handle().lock() {
                        buf_lines.push(Line::from(Span::styled(
                            format!("Worker {}/{}: launching...", i + 1, detected.total),
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                }
            }
        }
        let header_line = Line::from(vec![
            Span::styled("── WAVE: ", Style::default().fg(Color::Magenta)),
            Span::styled(
                format!(
                    "{} | {} workers | timeout {}s",
                    detected.hat_config.name, detected.total, wave_timeout_secs
                ),
                Style::default().fg(Color::Magenta),
            ),
            Span::styled(
                " ──────────────────────",
                Style::default().fg(Color::Magenta),
            ),
        ]);
        push_to_tui_iteration(state, header_line);
    }

    let main_events_file = resolve_current_events_path(ctx);
    let wave_result = execute_wave(
        &detected,
        backend,
        &main_events_file,
        out.show_cli,
        out.use_colors,
        out.rpc_tx.cloned(),
        out.tui.map(Arc::clone),
    )
    .await;

    match wave_result {
        Ok(completed) => {
            // Report completion to CLI / RPC / TUI
            if out.show_cli {
                print_wave_summary(
                    completed.results.len(),
                    completed.failures.len(),
                    completed.duration,
                    out.use_colors,
                );
            }
            if let Some(tx) = out.rpc_tx {
                let _ = tx.try_send(RpcEvent::WaveCompleted {
                    succeeded: completed.results.len(),
                    failed: completed.failures.len(),
                    duration_ms: completed.duration.as_millis() as u64,
                });
            }
            if let Some(state) = out.tui {
                if let Ok(mut s) = state.lock() {
                    let wave_iter_idx = s.wave_active_iteration_idx.take();
                    if let Some(wave) = s.wave_active.take() {
                        let target_idx =
                            wave_iter_idx.unwrap_or(s.iterations.len().saturating_sub(1));
                        if let Some(buf) = s.iterations.get_mut(target_idx) {
                            buf.wave_info = Some(wave);
                        }
                    }
                }
                let secs = completed.duration.as_secs();
                let color = if completed.failures.is_empty() {
                    Color::Green
                } else {
                    Color::Yellow
                };
                let line = Line::from(Span::styled(
                    format!(
                        "── Wave complete: {} succeeded, {} failed ({}s) ──────────────────────",
                        completed.results.len(),
                        completed.failures.len(),
                        secs,
                    ),
                    Style::default().fg(color),
                ));
                push_to_tui_iteration(state, line);
            }

            info!(
                wave_id = %completed.wave_id,
                results = completed.results.len(),
                failures = completed.failures.len(),
                duration_ms = completed.duration.as_millis() as u64,
                "Wave completed"
            );

            // Merge result events into main events file so aggregator hat picks them up
            if let Err(e) = merge_wave_results_to_events_file(
                &completed,
                &main_events_file,
                &detected.hat_config.publishes,
            ) {
                warn!(error = %e, "Failed to merge wave results to events file");
            }

            // Re-read events file to publish wave results to the bus.
            // The EventReader's position was before the merge, so it picks up
            // only the newly appended events (e.g. review.done). These target
            // the aggregator hat (concurrency=1), so they're partitioned as
            // regular events and published to the bus — making next_hat()
            // find the aggregator on the next iteration.
            if let Ok(reread) = event_loop.process_events_from_jsonl_with_waves()
                && reread.processed.had_events
            {
                info!("Published wave result events to bus for aggregator");
                // Wave results legitimately share the same topic (e.g.
                // 3x review.done). Reset the stale-loop counter so
                // this batch doesn't trigger LoopStale termination.
                event_loop.reset_stale_topic_counter();
            }
        }
        Err(e) => {
            warn!(error = %e, "Wave execution failed");
        }
    }
}

/// Execute a detected wave by spawning parallel backend instances.
///
/// Creates per-worker event files, spawns workers with concurrency-limited
/// semaphore, collects results, and returns a `CompletedWave`.
async fn execute_wave(
    wave: &ralph_core::DetectedWave,
    global_backend: &CliBackend,
    main_events_file: &Path,
    show_progress: bool,
    use_colors: bool,
    rpc_event_tx: Option<tokio::sync::mpsc::Sender<RpcEvent>>,
    tui_state: Option<Arc<std::sync::Mutex<ralph_tui::TuiState>>>,
) -> Result<ralph_core::CompletedWave> {
    use ralph_core::{WaveTracker, WaveWorkerContext, build_wave_worker_prompt};

    let concurrency = wave.hat_config.concurrency as usize;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

    let wave_timeout = Duration::from_secs(wave.timeout_secs());

    // Register wave in tracker
    let mut tracker = WaveTracker::new();
    tracker.register_wave(wave.wave_id.clone(), wave.total);

    // Resolve per-worker events directory
    let wave_dir = main_events_file
        .parent()
        .unwrap_or(Path::new(".ralph"))
        .to_path_buf();

    // Build payload previews for display (first ~60 chars of each event payload)
    let payload_previews: Vec<String> = wave
        .events
        .iter()
        .map(|e| e.payload.as_deref().unwrap_or("").replace('\n', " "))
        .collect();

    // Channel for real-time per-worker progress reporting
    let (progress_tx, mut progress_rx) =
        tokio::sync::mpsc::unbounded_channel::<(u32, bool, Duration)>();

    // Spawn workers
    let mut handles = Vec::new();
    for (index, event) in wave.events.iter().enumerate() {
        let permit = semaphore.clone().acquire_owned().await?;
        let wave_id = wave.wave_id.clone();
        let index = index as u32;
        let event = event.clone();
        let hat_config = wave.hat_config.clone();

        // Create per-worker events file
        let worker_events_file = wave_dir.join(format!("wave-{}-{}.jsonl", wave_id, index));

        // Build worker prompt
        let ctx = WaveWorkerContext {
            wave_id: wave_id.clone(),
            wave_index: index,
            wave_total: wave.total,
            result_topics: hat_config.publishes.clone(),
        };
        let prompt = build_wave_worker_prompt(&hat_config, &event, &ctx);

        // Resolve backend for this worker
        let mut worker_backend = if let Some(ref hat_backend) = hat_config.backend {
            CliBackend::from_hat_backend(hat_backend).unwrap_or_else(|_| global_backend.clone())
        } else {
            global_backend.clone()
        };

        // Inject wave env vars
        worker_backend.env_vars.extend([
            ("RALPH_WAVE_WORKER".into(), "1".into()),
            ("RALPH_WAVE_ID".into(), wave_id.clone()),
            ("RALPH_WAVE_INDEX".into(), index.to_string()),
            (
                "RALPH_EVENTS_FILE".into(),
                worker_events_file.display().to_string(),
            ),
        ]);

        // Apply hat backend args
        if let Some(ref args) = hat_config.backend_args {
            worker_backend.args.extend(args.iter().cloned());
        }

        let worker_events_path = worker_events_file.clone();
        let tx = progress_tx.clone();
        let worker_rpc_tx = rpc_event_tx.clone();
        let worker_tui_state = tui_state.clone();

        let handle = tokio::spawn(async move {
            let _permit = permit; // Hold permit for concurrency limiting
            run_wave_worker(
                index,
                &worker_backend,
                &prompt,
                &worker_events_path,
                wave_timeout,
                tx,
                worker_rpc_tx,
                worker_tui_state,
            )
            .await
        });

        handles.push(handle);
    }

    // Drop our sender so the receiver terminates when all workers finish
    drop(progress_tx);

    // Spawn a task to report real-time progress (CLI, RPC, and/or TUI)
    let total = wave.total;
    let previews = payload_previews;
    let progress_handle = tokio::spawn(async move {
        while let Some((index, success, duration)) = progress_rx.recv().await {
            let preview = previews
                .get(index as usize)
                .map(|s| s.as_str())
                .unwrap_or("");
            if show_progress {
                print_wave_worker_done(index, total, duration, success, preview, use_colors);
            }
            if let Some(ref tx) = rpc_event_tx {
                let _ = tx.try_send(RpcEvent::WaveWorkerDone {
                    index,
                    total,
                    duration_ms: duration.as_millis() as u64,
                    success,
                    payload_preview: preview.to_string(),
                });
            }
            if let Some(ref state) = tui_state {
                if let Ok(mut s) = state.lock()
                    && let Some(ref mut wave) = s.wave_active
                {
                    wave.completed += 1;
                }
                let secs = duration.as_secs();
                let (icon, color) = if success {
                    ("\u{2713}", Color::Green)
                } else {
                    ("\u{2717}", Color::Red)
                };
                let status_word = if success { "done" } else { "failed" };
                let truncated_preview = if preview.len() > 60 {
                    &preview[..ralph_core::floor_char_boundary(preview, 60)]
                } else {
                    preview
                };
                let line = Line::from(vec![
                    Span::styled(format!("  {} ", icon), Style::default().fg(color)),
                    Span::raw(format!(
                        "Worker {}/{} {} ({}s) — {}",
                        index + 1,
                        total,
                        status_word,
                        secs,
                        truncated_preview
                    )),
                ]);
                push_to_tui_iteration(state, line);
            }
        }
    });

    // Compute aggregate timeout: enough wall-clock time for all batches + buffer.
    // With semaphore-based concurrency limiting, total time is bounded by
    // ceil(total / concurrency) * per_worker_timeout.
    let batches = u64::from(wave.total).div_ceil(concurrency as u64);
    let aggregate_timeout = Duration::from_secs(wave_timeout.as_secs().saturating_mul(batches))
        + Duration::from_secs(30);

    // Collect results with aggregate timeout to prevent indefinite hangs
    let results =
        match tokio::time::timeout(aggregate_timeout, futures::future::join_all(handles)).await {
            Ok(results) => results,
            Err(_) => {
                warn!(
                    timeout_secs = aggregate_timeout.as_secs(),
                    "Wave aggregate timeout reached, cancelling remaining workers"
                );
                Vec::new()
            }
        };

    // Wait for progress reporter to finish
    let _ = progress_handle.await;

    let mut reported_indices = std::collections::HashSet::new();

    for result in results {
        match result {
            Ok((index, Ok((events, _duration, _success)))) => {
                reported_indices.insert(index);
                let proto_events: Vec<ralph_proto::Event> =
                    events.into_iter().map(ralph_proto::Event::from).collect();
                tracker.record_result(&wave.wave_id, index, proto_events);
            }
            Ok((index, Err((error, duration)))) => {
                reported_indices.insert(index);
                tracker.record_failure(&wave.wave_id, index, error, duration);
            }
            Err(join_err) => {
                // Task panicked or was cancelled — index is lost from JoinError.
                // The missing-index sweep below will record the failure.
                warn!(error = %join_err, "Wave worker task panicked");
            }
        }
    }

    // Record failures for any workers that didn't report back (panicked or timed out).
    for i in 0..wave.total {
        if !reported_indices.contains(&i) {
            warn!(
                worker = i,
                wave_id = %wave.wave_id,
                "Worker did not report — recording synthetic failure"
            );
            tracker.record_failure(
                &wave.wave_id,
                i,
                "Worker panicked or was cancelled by aggregate timeout".into(),
                aggregate_timeout,
            );
        }
    }

    // Take completed wave results
    tracker
        .take_wave_results(&wave.wave_id)
        .ok_or_else(|| anyhow::anyhow!("Wave {} not found in tracker", wave.wave_id))
}

type WaveWorkerOutcome =
    std::result::Result<(Vec<ralph_core::Event>, Duration, bool), (String, Duration)>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WaveWorkerExecutionMode {
    Pty,
    Acp,
}

fn wave_worker_execution_mode(output_format: BackendOutputFormat) -> WaveWorkerExecutionMode {
    match output_format {
        BackendOutputFormat::Acp => WaveWorkerExecutionMode::Acp,
        _ => WaveWorkerExecutionMode::Pty,
    }
}

struct WaveWorkerStreamHandler {
    worker_index: u32,
    rpc_tx: Option<tokio::sync::mpsc::Sender<RpcEvent>>,
    tui_state: Option<Arc<std::sync::Mutex<ralph_tui::TuiState>>>,
}

impl WaveWorkerStreamHandler {
    fn new(
        worker_index: u32,
        rpc_tx: Option<tokio::sync::mpsc::Sender<RpcEvent>>,
        tui_state: Option<Arc<std::sync::Mutex<ralph_tui::TuiState>>>,
    ) -> Self {
        Self {
            worker_index,
            rpc_tx,
            tui_state,
        }
    }

    fn emit_delta(&self, delta: impl Into<String>) {
        let delta = delta.into();
        if delta.is_empty() {
            return;
        }

        if let Some(ref rpc_tx) = self.rpc_tx {
            let _ = rpc_tx.try_send(RpcEvent::WaveWorkerTextDelta {
                worker_index: self.worker_index,
                delta: delta.clone(),
            });
        }

        if let Some(ref state) = self.tui_state {
            let tui_lines = ralph_tui::text_to_lines(&delta);
            push_to_wave_worker_buffer(state, self.worker_index as usize, &tui_lines);
        }
    }
}

impl StreamHandler for WaveWorkerStreamHandler {
    fn on_text(&mut self, text: &str) {
        self.emit_delta(text);
    }

    fn on_tool_call(&mut self, name: &str, _id: &str, input: &serde_json::Value) {
        self.emit_delta(format!("⚙ {name}({input})\n"));
    }

    fn on_tool_result(&mut self, _id: &str, output: &str) {
        if output.is_empty() {
            return;
        }
        self.emit_delta(format!("→ {}\n", truncate_wave_worker_preview(output)));
    }

    fn on_error(&mut self, error: &str) {
        if error.is_empty() {
            return;
        }
        self.emit_delta(format!("✗ {}\n", truncate_wave_worker_preview(error)));
    }

    fn on_complete(&mut self, _result: &ralph_adapters::SessionResult) {}
}

async fn run_wave_worker(
    index: u32,
    worker_backend: &CliBackend,
    prompt: &str,
    worker_events_path: &Path,
    wave_timeout: Duration,
    tx: tokio::sync::mpsc::UnboundedSender<(u32, bool, Duration)>,
    worker_rpc_tx: Option<tokio::sync::mpsc::Sender<RpcEvent>>,
    worker_tui_state: Option<Arc<std::sync::Mutex<ralph_tui::TuiState>>>,
) -> (u32, WaveWorkerOutcome) {
    match wave_worker_execution_mode(worker_backend.output_format) {
        WaveWorkerExecutionMode::Pty => {
            run_wave_worker_pty(
                index,
                worker_backend,
                prompt,
                worker_events_path,
                wave_timeout,
                tx,
                worker_rpc_tx,
                worker_tui_state,
            )
            .await
        }
        WaveWorkerExecutionMode::Acp => {
            run_wave_worker_acp(
                index,
                worker_backend,
                prompt,
                worker_events_path,
                wave_timeout,
                tx,
                worker_rpc_tx,
                worker_tui_state,
            )
            .await
        }
    }
}

enum AcpWaveExecutionResult {
    Completed(std::result::Result<bool, String>),
    TimedOut,
}

#[cfg(test)]
#[derive(Clone, Debug)]
enum MockAcpExecution {
    Success {
        success: bool,
        events: Vec<ralph_core::Event>,
    },
    Error {
        error: String,
        events: Vec<ralph_core::Event>,
    },
    Timeout {
        events: Vec<ralph_core::Event>,
    },
}

#[cfg(test)]
impl MockAcpExecution {
    fn success(success: bool, events: Vec<ralph_core::Event>) -> Self {
        Self::Success { success, events }
    }

    fn error(error: impl Into<String>, events: Vec<ralph_core::Event>) -> Self {
        Self::Error {
            error: error.into(),
            events,
        }
    }

    fn timeout(events: Vec<ralph_core::Event>) -> Self {
        Self::Timeout { events }
    }

    fn write_capture(&self, worker_backend: &CliBackend, prompt: &str, worker_events_path: &Path) {
        let capture_path = PathBuf::from(format!("{}.capture", worker_events_path.display()));
        let env = worker_backend
            .env_vars
            .iter()
            .cloned()
            .collect::<std::collections::BTreeMap<_, _>>();
        let capture = serde_json::json!({
            "command": worker_backend.command.as_str(),
            "args": &worker_backend.args,
            "env": env,
            "prompt": prompt,
        });
        std::fs::write(
            &capture_path,
            serde_json::to_string(&capture).expect("serialize mock ACP invocation"),
        )
        .expect("write mock ACP invocation");
    }

    fn write_events(&self, worker_events_path: &Path) {
        let events = match self {
            Self::Success { events, .. }
            | Self::Error { events, .. }
            | Self::Timeout { events } => events,
        };

        if events.is_empty() {
            let _ = fs::remove_file(worker_events_path);
            return;
        }

        let content = events
            .iter()
            .map(|event| serde_json::to_string(event).expect("serialize mock ACP event"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(worker_events_path, format!("{content}\n")).expect("write mock ACP events");
    }
}

#[cfg(test)]
static MOCK_ACP_EXECUTIONS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::VecDeque<MockAcpExecution>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::VecDeque::new()));

#[cfg(test)]
static MOCK_ACP_EXECUTION_SERIAL: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

async fn execute_wave_worker_acp_prompt(
    index: u32,
    worker_backend: &CliBackend,
    prompt: &str,
    _worker_events_path: &Path,
    wave_timeout: Duration,
    worker_rpc_tx: Option<tokio::sync::mpsc::Sender<RpcEvent>>,
    worker_tui_state: Option<Arc<std::sync::Mutex<ralph_tui::TuiState>>>,
) -> AcpWaveExecutionResult {
    #[cfg(test)]
    {
        if let Some(mock) = {
            let mut queued = MOCK_ACP_EXECUTIONS.lock().expect("mock ACP execution lock");
            queued.pop_front()
        } {
            mock.write_capture(worker_backend, prompt, _worker_events_path);
            mock.write_events(_worker_events_path);
            return match mock {
                MockAcpExecution::Success { success, .. } => {
                    AcpWaveExecutionResult::Completed(Ok(success))
                }
                MockAcpExecution::Error { error, .. } => {
                    AcpWaveExecutionResult::Completed(Err(error))
                }
                MockAcpExecution::Timeout { .. } => AcpWaveExecutionResult::TimedOut,
            };
        }
    }

    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let executor = AcpExecutor::new(worker_backend.clone(), workspace_root);
    let mut handler = WaveWorkerStreamHandler::new(index, worker_rpc_tx, worker_tui_state);

    match tokio::time::timeout(wave_timeout, executor.execute(prompt, &mut handler)).await {
        Ok(Ok(result)) => AcpWaveExecutionResult::Completed(Ok(result.success)),
        Ok(Err(e)) => AcpWaveExecutionResult::Completed(Err(e.to_string())),
        Err(_) => AcpWaveExecutionResult::TimedOut,
    }
}

async fn run_wave_worker_acp(
    index: u32,
    worker_backend: &CliBackend,
    prompt: &str,
    worker_events_path: &Path,
    wave_timeout: Duration,
    tx: tokio::sync::mpsc::UnboundedSender<(u32, bool, Duration)>,
    worker_rpc_tx: Option<tokio::sync::mpsc::Sender<RpcEvent>>,
    worker_tui_state: Option<Arc<std::sync::Mutex<ralph_tui::TuiState>>>,
) -> (u32, WaveWorkerOutcome) {
    let start = std::time::Instant::now();
    let result = execute_wave_worker_acp_prompt(
        index,
        worker_backend,
        prompt,
        worker_events_path,
        wave_timeout,
        worker_rpc_tx,
        worker_tui_state,
    )
    .await;
    let duration = start.elapsed();
    let events = read_worker_events(worker_events_path);
    let _ = fs::remove_file(worker_events_path);

    match result {
        AcpWaveExecutionResult::Completed(Ok(success)) => {
            let _ = tx.send((index, success, duration));
            (index, Ok((events, duration, success)))
        }
        AcpWaveExecutionResult::Completed(Err(error)) => {
            let _ = tx.send((index, false, duration));
            (
                index,
                Err((format!("ACP worker failed: {error}"), duration)),
            )
        }
        AcpWaveExecutionResult::TimedOut if events.is_empty() => {
            let _ = tx.send((index, false, duration));
            (
                index,
                Err((
                    format!(
                        "Worker timed out after {}s without emitting events",
                        wave_timeout.as_secs()
                    ),
                    duration,
                )),
            )
        }
        AcpWaveExecutionResult::TimedOut => {
            let _ = tx.send((index, false, duration));
            (index, Ok((events, duration, false)))
        }
    }
}

#[cfg(test)]
fn forced_test_wave_pty_failure<'a>(worker_backend: &'a CliBackend, key: &str) -> Option<&'a str> {
    worker_backend
        .env_vars
        .iter()
        .find_map(|(name, value)| (name == key).then_some(value.as_str()))
}

async fn run_wave_worker_pty(
    index: u32,
    worker_backend: &CliBackend,
    prompt: &str,
    worker_events_path: &Path,
    wave_timeout: Duration,
    tx: tokio::sync::mpsc::UnboundedSender<(u32, bool, Duration)>,
    worker_rpc_tx: Option<tokio::sync::mpsc::Sender<RpcEvent>>,
    worker_tui_state: Option<Arc<std::sync::Mutex<ralph_tui::TuiState>>>,
) -> (u32, WaveWorkerOutcome) {
    let start = std::time::Instant::now();

    // Build and spawn process in a PTY for real-time stdout streaming.
    // Node.js structured backends buffer stdout when it's a pipe, so NDJSON
    // events only arrive when the process exits. Using a PTY forces the
    // child to see a terminal and flush after each line.
    let (cmd, args, stdin_input, _temp_file_guard) = worker_backend.build_command(prompt, false);
    let mut stdin_prompt_file = None;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let output_format = worker_backend.output_format;

    #[cfg(test)]
    if let Some(error) =
        forced_test_wave_pty_failure(worker_backend, "RALPH_TEST_FORCE_PTY_OPEN_FAIL")
    {
        let duration = start.elapsed();
        let _ = fs::remove_file(worker_events_path);
        let _ = tx.send((index, false, duration));
        return (index, Err((format!("PTY open failed: {error}"), duration)));
    }

    // Spawn worker in a PTY so stdout is unbuffered
    let pty_system = portable_pty::native_pty_system();
    let pty_pair = match pty_system.openpty(portable_pty::PtySize {
        rows: 24,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(pair) => pair,
        Err(e) => {
            let duration = start.elapsed();
            let _ = fs::remove_file(worker_events_path);
            let _ = tx.send((index, false, duration));
            return (index, Err((format!("PTY open failed: {e}"), duration)));
        }
    };

    let (spawn_cmd, spawn_args) = if let Some(input) = stdin_input.as_ref() {
        let mut prompt_file = match tempfile::NamedTempFile::new() {
            Ok(file) => file,
            Err(e) => {
                let duration = start.elapsed();
                let _ = fs::remove_file(worker_events_path);
                let _ = tx.send((index, false, duration));
                return (
                    index,
                    Err((
                        format!("PTY stdin temp file creation failed: {e}"),
                        duration,
                    )),
                );
            }
        };
        if let Err(e) = std::io::Write::write_all(&mut prompt_file, input.as_bytes()) {
            let duration = start.elapsed();
            let _ = fs::remove_file(worker_events_path);
            let _ = tx.send((index, false, duration));
            return (
                index,
                Err((format!("PTY stdin temp file write failed: {e}"), duration)),
            );
        }

        let wrapper_args = std::iter::once("-c".to_string())
            .chain(std::iter::once(
                r#"prompt_file="$1"; shift; exec "$@" < "$prompt_file""#.to_string(),
            ))
            .chain(std::iter::once("sh".to_string()))
            .chain(std::iter::once(prompt_file.path().display().to_string()))
            .chain(std::iter::once(cmd.clone()))
            .chain(args.iter().cloned())
            .collect::<Vec<_>>();
        stdin_prompt_file = Some(prompt_file);
        ("sh".to_string(), wrapper_args)
    } else {
        (cmd.clone(), args.clone())
    };

    let mut cmd_builder = portable_pty::CommandBuilder::new(&spawn_cmd);
    cmd_builder.args(&spawn_args);
    cmd_builder.cwd(&cwd);
    for (key, value) in &worker_backend.env_vars {
        cmd_builder.env(key, value);
    }
    cmd_builder.env("TERM", "dumb");
    cmd_builder.env("NO_COLOR", "1");

    let mut child = match pty_pair.slave.spawn_command(cmd_builder) {
        Ok(child) => child,
        Err(e) => {
            let duration = start.elapsed();
            let _ = fs::remove_file(worker_events_path);
            let _ = tx.send((index, false, duration));
            return (index, Err((format!("PTY spawn failed: {e}"), duration)));
        }
    };
    drop(pty_pair.slave);

    if let Some(input) = stdin_input
        && stdin_prompt_file.is_none()
        && let Ok(mut writer) = pty_pair.master.take_writer()
    {
        let _ = writer.write_all(input.as_bytes());
        let _ = writer.write_all(b"\n");
        let _ = writer.flush();
    }

    #[cfg(test)]
    if let Some(error) =
        forced_test_wave_pty_failure(worker_backend, "RALPH_TEST_FORCE_PTY_READER_FAIL")
    {
        let duration = start.elapsed();
        let _ = fs::remove_file(worker_events_path);
        let _ = tx.send((index, false, duration));
        return (
            index,
            Err((format!("PTY reader failed: {error}"), duration)),
        );
    }

    let pty_reader = match pty_pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(e) => {
            let duration = start.elapsed();
            let _ = fs::remove_file(worker_events_path);
            let _ = tx.send((index, false, duration));
            return (index, Err((format!("PTY reader failed: {e}"), duration)));
        }
    };

    let (line_tx, mut line_rx) = tokio::sync::mpsc::channel::<String>(256);
    let reader_handle = std::thread::spawn(move || {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(pty_reader);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if line_tx.blocking_send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut timed_out = false;
    let stream_result = async {
        let mut line_count: u64 = 0;
        while let Some(line) = line_rx.recv().await {
            line_count += 1;
            if line_count == 1 {
                info!(
                    worker = index,
                    line_len = line.len(),
                    ?output_format,
                    "Wave worker: first stdout line received"
                );
            }
            if let Some(delta) = extract_readable_delta(&line, output_format) {
                if let Some(ref rpc_tx) = worker_rpc_tx {
                    let _ = rpc_tx.try_send(RpcEvent::WaveWorkerTextDelta {
                        worker_index: index,
                        delta: delta.clone(),
                    });
                }
                if let Some(ref state) = worker_tui_state {
                    let tui_lines = ralph_tui::text_to_lines(&delta);
                    push_to_wave_worker_buffer(state, index as usize, &tui_lines);
                }
            }
        }
        Ok::<_, std::io::Error>(())
    };

    match tokio::time::timeout(wave_timeout, stream_result).await {
        Ok(result) => {
            if let Err(e) = result {
                warn!(error = %e, worker = index, "Wave worker I/O error");
            }
        }
        Err(_) => {
            warn!(
                timeout_secs = wave_timeout.as_secs(),
                worker = index,
                "Wave worker timeout, killing process"
            );
            timed_out = true;
            let _ = child.kill();
        }
    }

    let (status, _) = tokio::task::spawn_blocking(move || {
        let status = child.wait();
        let _ = reader_handle.join();
        (status, ())
    })
    .await
    .unwrap_or_else(|_| (Err(std::io::Error::other("join task panicked")), ()));
    let success = status.map(|s| s.success() && !timed_out).unwrap_or(false);
    let duration = start.elapsed();

    let events = read_worker_events(worker_events_path);
    let _ = fs::remove_file(worker_events_path);

    if timed_out && events.is_empty() {
        let _ = tx.send((index, false, duration));
        (
            index,
            Err((
                format!(
                    "Worker timed out after {}s without emitting events",
                    wave_timeout.as_secs()
                ),
                duration,
            )),
        )
    } else {
        let _ = tx.send((index, success, duration));
        (index, Ok((events, duration, success)))
    }
}

fn truncate_wave_worker_preview(text: &str) -> String {
    if text.len() > 200 {
        let end = ralph_core::floor_char_boundary(text, 200);
        format!("{}…", &text[..end])
    } else {
        text.to_string()
    }
}

/// Extract a human-readable text delta from a single stdout line.
fn extract_readable_delta(line: &str, output_format: BackendOutputFormat) -> Option<String> {
    match output_format {
        BackendOutputFormat::Text | BackendOutputFormat::Acp => Some(format!("{line}\n")),
        BackendOutputFormat::StreamJson => {
            use ralph_adapters::{ClaudeStreamEvent, ClaudeStreamParser, ContentBlock};
            match ClaudeStreamParser::parse_line(line) {
                Some(ClaudeStreamEvent::Assistant { message, .. }) => {
                    let mut text = String::new();
                    for block in message.content {
                        match block {
                            ContentBlock::Text { text: t } => {
                                text.push_str(&t);
                                text.push('\n');
                            }
                            ContentBlock::ToolUse { name, input, .. } => {
                                text.push_str(&format!("⚙ {name}({input})\n"));
                            }
                        }
                    }
                    if text.is_empty() { None } else { Some(text) }
                }
                Some(ClaudeStreamEvent::User { message }) => {
                    let mut text = String::new();
                    for block in message.content {
                        let ralph_adapters::UserContentBlock::ToolResult { content, .. } = block;
                        if !content.is_empty() {
                            text.push_str(&format!(
                                "→ {}\n",
                                truncate_wave_worker_preview(&content)
                            ));
                        }
                    }
                    if text.is_empty() { None } else { Some(text) }
                }
                _ => None,
            }
        }
        BackendOutputFormat::PiStreamJson => match PiStreamParser::parse_line(line) {
            Some(PiStreamEvent::MessageUpdate {
                assistant_message_event: PiAssistantEvent::TextDelta { delta },
            }) => Some(delta),
            Some(PiStreamEvent::MessageUpdate {
                assistant_message_event: PiAssistantEvent::Error { reason },
            }) => Some(format!("✗ {}\n", truncate_wave_worker_preview(&reason))),
            Some(PiStreamEvent::ToolExecutionStart {
                tool_name, args, ..
            }) => Some(format!("⚙ {tool_name}({args})\n")),
            Some(PiStreamEvent::ToolExecutionEnd {
                result, is_error, ..
            }) => {
                let output = result
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        PiContentBlock::Text { text } => Some(text.as_str()),
                        PiContentBlock::Other => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if output.is_empty() {
                    None
                } else if is_error {
                    Some(format!("✗ {}\n", truncate_wave_worker_preview(&output)))
                } else {
                    Some(format!("→ {}\n", truncate_wave_worker_preview(&output)))
                }
            }
            _ => None,
        },
    }
}

/// Read events from a per-worker events file.
fn read_worker_events(path: &Path) -> Vec<ralph_core::Event> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };

    content
        .lines()
        .filter_map(|line| serde_json::from_str::<ralph_core::Event>(line).ok())
        .collect()
}

/// Merge wave result events into the main events file.
///
/// Appends all result events to the main JSONL file so the aggregator hat
/// picks them up on the next iteration.
fn merge_wave_results_to_events_file(
    completed: &ralph_core::CompletedWave,
    events_file: &Path,
    publish_topics: &[String],
) -> Result<()> {
    use std::io::Write;

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_file)
        .with_context(|| format!("Failed to open events file: {}", events_file.display()))?;

    let ts = chrono::Utc::now().to_rfc3339();

    // Build all records into a single buffer, then write_all once for atomic
    // append (consistent with EventLogger::log and write_wave_events).
    let mut buf = String::new();

    for result in &completed.results {
        for event in &result.events {
            let record = serde_json::json!({
                "topic": event.topic.as_str(),
                "payload": event.payload,
                "ts": ts,
                "wave_id": completed.wave_id,
                "wave_index": result.index,
            });
            buf.push_str(&serde_json::to_string(&record)?);
            buf.push('\n');
        }
    }

    // Also write failure events so the aggregator knows about partial results
    for failure in &completed.failures {
        let record = serde_json::json!({
            "topic": "wave.worker.failed",
            "payload": format!("Worker {} failed: {}", failure.index, failure.error),
            "ts": ts,
            "wave_id": completed.wave_id,
            "wave_index": failure.index,
        });
        buf.push_str(&serde_json::to_string(&record)?);
        buf.push('\n');

        // Emit synthetic events on the hat's publish topics so downstream
        // aggregators can still trigger even when workers fail/timeout
        for topic in publish_topics {
            let record = serde_json::json!({
                "topic": topic,
                "payload": format!(
                    "## Worker {} (FAILED)\n\nError: {}",
                    failure.index, failure.error
                ),
                "ts": ts,
                "wave_id": completed.wave_id,
                "wave_index": failure.index,
            });
            buf.push_str(&serde_json::to_string(&record)?);
            buf.push('\n');
        }
    }

    file.write_all(buf.as_bytes())?;

    Ok(())
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
    fn test_resolve_loop_id_fresh_generates_new() {
        let temp = tempfile::TempDir::new().unwrap();
        let ctx = ralph_core::LoopContext::primary(temp.path().to_path_buf());
        ctx.ensure_ralph_dir().unwrap();

        let id = resolve_loop_id(&ctx, false, None);
        assert!(
            id.starts_with("primary-"),
            "Fresh run should generate primary-{{timestamp}}, got: {}",
            id
        );
    }

    #[test]
    fn test_resolve_loop_id_continue_reuses_marker() {
        let temp = tempfile::TempDir::new().unwrap();
        let ctx = ralph_core::LoopContext::primary(temp.path().to_path_buf());
        ctx.ensure_ralph_dir().unwrap();

        // Write a marker from a "previous run"
        std::fs::write(
            ctx.ralph_dir().join("current-loop-id"),
            "primary-20260303-100000",
        )
        .unwrap();

        let id = resolve_loop_id(&ctx, true, None);
        assert_eq!(
            id, "primary-20260303-100000",
            "--continue should reuse existing loop ID"
        );
    }

    #[test]
    fn test_resolve_loop_id_continue_explicit_overrides_marker() {
        let temp = tempfile::TempDir::new().unwrap();
        let ctx = ralph_core::LoopContext::primary(temp.path().to_path_buf());
        ctx.ensure_ralph_dir().unwrap();

        std::fs::write(
            ctx.ralph_dir().join("current-loop-id"),
            "primary-20260303-100000",
        )
        .unwrap();

        let id = resolve_loop_id(&ctx, true, Some("custom-loop-42"));
        assert_eq!(
            id, "custom-loop-42",
            "--loop-id should override the marker file"
        );
    }

    #[test]
    fn test_resolve_loop_id_continue_no_marker_generates_new() {
        let temp = tempfile::TempDir::new().unwrap();
        let ctx = ralph_core::LoopContext::primary(temp.path().to_path_buf());
        ctx.ensure_ralph_dir().unwrap();

        // No marker file exists
        let id = resolve_loop_id(&ctx, true, None);
        assert!(
            id.starts_with("primary-"),
            "--continue without marker should fall back to generating new ID, got: {}",
            id
        );
    }

    #[test]
    fn test_pty_only_enabled_for_tui_rpc_or_interactive() {
        let should_use_pty = |enable_tui: bool, enable_rpc: bool, user_interactive: bool| -> bool {
            enable_tui || enable_rpc || user_interactive
        };

        assert!(!should_use_pty(false, false, false));
        assert!(should_use_pty(true, false, false));
        assert!(should_use_pty(false, true, false));
        assert!(should_use_pty(false, false, true));
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

    #[cfg(unix)]
    static FAKE_PATH_BACKEND_SERIAL: std::sync::LazyLock<std::sync::Mutex<()>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

    #[cfg(unix)]
    struct FakePathBackendsGuard {
        _guard: std::sync::MutexGuard<'static, ()>,
        installed_paths: Vec<std::path::PathBuf>,
    }

    #[cfg(unix)]
    impl Drop for FakePathBackendsGuard {
        fn drop(&mut self) {
            for path in &self.installed_paths {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    #[cfg(unix)]
    fn fake_path_backend_dir(backends: &[(&str, &str)]) -> std::path::PathBuf {
        use std::fs::OpenOptions;

        let path = std::env::var_os("PATH").unwrap_or_default();
        for dir in std::env::split_paths(&path) {
            if !dir.is_dir() {
                continue;
            }
            if backends.iter().any(|(name, _)| dir.join(name).exists()) {
                continue;
            }

            let probe = dir.join(format!(".ralph-fake-path-probe-{}", std::process::id()));
            if OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&probe)
                .is_ok()
            {
                let _ = std::fs::remove_file(&probe);
                return dir;
            }
        }

        panic!("expected at least one writable PATH entry for fake backend installation");
    }

    #[cfg(unix)]
    fn install_fake_path_backends(backends: &[(&str, &str)]) -> FakePathBackendsGuard {
        let guard = FAKE_PATH_BACKEND_SERIAL
            .lock()
            .expect("fake PATH backend serial lock");
        let bin_dir = fake_path_backend_dir(backends);

        let mut installed_paths = Vec::with_capacity(backends.len());
        for (name, body) in backends {
            let path = bin_dir.join(name);
            assert!(
                !path.exists(),
                "expected fake backend slot to be free: {}",
                path.display()
            );
            installed_paths.push(write_fake_executable(&bin_dir, name, body));
        }

        FakePathBackendsGuard {
            _guard: guard,
            installed_paths,
        }
    }

    #[cfg(test)]
    struct MockAcpExecutionGuard {
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    #[cfg(test)]
    impl Drop for MockAcpExecutionGuard {
        fn drop(&mut self) {
            MOCK_ACP_EXECUTIONS
                .lock()
                .expect("mock ACP execution queue")
                .clear();
        }
    }

    #[cfg(test)]
    fn install_mock_acp_executions(executions: Vec<MockAcpExecution>) -> MockAcpExecutionGuard {
        let guard = MOCK_ACP_EXECUTION_SERIAL
            .lock()
            .expect("mock ACP execution serial lock");
        *MOCK_ACP_EXECUTIONS
            .lock()
            .expect("mock ACP execution queue") = executions.into_iter().collect();
        MockAcpExecutionGuard { _guard: guard }
    }

    #[cfg(unix)]
    fn hook_spec_with_command_and_on_error_and_suspend_mode(
        name: &str,
        command: Vec<String>,
        on_error: HookOnError,
        suspend_mode: Option<HookSuspendMode>,
    ) -> ralph_core::HookSpec {
        ralph_core::HookSpec {
            name: name.to_string(),
            command,
            cwd: None,
            env: std::collections::HashMap::new(),
            timeout_seconds: None,
            max_output_bytes: None,
            on_error: Some(on_error),
            suspend_mode,
            mutate: ralph_core::HookMutationConfig::default(),
            extra: std::collections::HashMap::new(),
        }
    }

    #[cfg(unix)]
    fn hook_spec_with_command_and_on_error(
        name: &str,
        command: Vec<String>,
        on_error: HookOnError,
    ) -> ralph_core::HookSpec {
        hook_spec_with_command_and_on_error_and_suspend_mode(name, command, on_error, None)
    }

    #[cfg(unix)]
    fn hook_spec_with_command(name: &str, command: Vec<String>) -> ralph_core::HookSpec {
        hook_spec_with_command_and_on_error(name, command, HookOnError::Warn)
    }

    #[cfg(unix)]
    fn recording_hook(name: &str, log_path: &Path) -> ralph_core::HookSpec {
        hook_spec_with_command(
            name,
            vec![
                "sh".to_string(),
                "-c".to_string(),
                r#"payload="$(cat)"
phase="$(printf '%s' "$payload" | grep -o '"phase_event":"[^"]*"' | cut -d'"' -f4)"
printf '%s|%s\n' "$1" "$phase" >> "$2""#
                    .to_string(),
                "hook-recorder".to_string(),
                name.to_string(),
                log_path.to_string_lossy().into_owned(),
            ],
        )
    }

    #[cfg(unix)]
    fn payload_recording_hook(name: &str, log_path: &Path) -> ralph_core::HookSpec {
        hook_spec_with_command(
            name,
            vec![
                "sh".to_string(),
                "-c".to_string(),
                r#"payload="$(cat)"
printf '%s\n' "$payload" >> "$1""#
                    .to_string(),
                "hook-payload-recorder".to_string(),
                log_path.to_string_lossy().into_owned(),
            ],
        )
    }

    #[cfg(unix)]
    fn hook_engine_with_events(
        events: std::collections::HashMap<HookPhaseEvent, Vec<ralph_core::HookSpec>>,
    ) -> HookEngine {
        let hooks_config = ralph_core::HooksConfig {
            enabled: true,
            events,
            ..ralph_core::HooksConfig::default()
        };
        HookEngine::new(&hooks_config)
    }

    #[cfg(unix)]
    fn dispatch_test_event_loop(workspace_root: &Path) -> EventLoop {
        let mut config = RalphConfig::default();
        config.core.workspace_root = workspace_root.to_path_buf();
        EventLoop::new(config)
    }

    #[cfg(unix)]
    fn dispatch_test_event_loop_with_context(workspace_root: &Path) -> (EventLoop, LoopContext) {
        let mut config = RalphConfig::default();
        config.core.workspace_root = workspace_root.to_path_buf();
        let context = LoopContext::primary(workspace_root.to_path_buf());
        let event_loop = EventLoop::with_context(config, context.clone());
        (event_loop, context)
    }

    fn dispatch_test_event_loop_from_yaml_with_context(
        workspace_root: &Path,
        yaml: &str,
    ) -> (EventLoop, LoopContext) {
        let mut config: RalphConfig = serde_yaml::from_str(yaml).expect("parse config");
        config.core.workspace_root = workspace_root.to_path_buf();
        let context = LoopContext::primary(workspace_root.to_path_buf());
        let event_loop = EventLoop::with_context(config, context.clone());
        (event_loop, context)
    }

    #[cfg(unix)]
    fn dispatch_test_event_loop_with_diagnostics(workspace_root: &Path) -> EventLoop {
        let mut config = RalphConfig::default();
        config.core.workspace_root = workspace_root.to_path_buf();
        let diagnostics =
            ralph_core::diagnostics::DiagnosticsCollector::with_enabled(workspace_root, true)
                .expect("create diagnostics collector");
        EventLoop::with_diagnostics(config, diagnostics)
    }

    #[cfg(unix)]
    fn read_hook_run_telemetry_entries(workspace_root: &Path) -> Vec<HookRunTelemetryEntry> {
        let diagnostics_root = workspace_root.join(".ralph").join("diagnostics");
        let mut session_dirs: Vec<_> = std::fs::read_dir(&diagnostics_root)
            .expect("read diagnostics root")
            .filter_map(Result::ok)
            .collect();
        session_dirs.sort_by_key(|entry| entry.path());

        let latest_session = session_dirs
            .last()
            .expect("at least one diagnostics session should exist");
        let hook_runs_path = latest_session.path().join("hook-runs.jsonl");
        let content = std::fs::read_to_string(&hook_runs_path).expect("read hook-runs.jsonl");

        content
            .lines()
            .map(|line| serde_json::from_str(line).expect("parse hook run telemetry entry"))
            .collect()
    }

    #[cfg(unix)]
    fn read_hook_log(log_path: &Path) -> Vec<String> {
        std::fs::read_to_string(log_path)
            .expect("read hook log")
            .lines()
            .map(str::to_string)
            .collect()
    }

    #[cfg(unix)]
    fn read_hook_payload_log(log_path: &Path) -> Vec<serde_json::Value> {
        std::fs::read_to_string(log_path)
            .expect("read hook payload log")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("parse hook payload JSON"))
            .collect()
    }

    fn suspend_outcome_with_mode(
        phase_event: HookPhaseEvent,
        hook_name: &str,
        suspend_mode: HookSuspendMode,
    ) -> HookDispatchOutcome {
        HookDispatchOutcome {
            phase_event,
            hook_name: hook_name.to_string(),
            disposition: HookDisposition::Suspend,
            suspend_mode,
            failure: Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(41),
                timed_out: false,
            }),

            mutation_parse_outcome: HookMutationParseOutcome::Disabled,
        }
    }

    fn suspend_outcome(phase_event: HookPhaseEvent, hook_name: &str) -> HookDispatchOutcome {
        suspend_outcome_with_mode(phase_event, hook_name, HookSuspendMode::WaitForResume)
    }

    fn block_on_test_future<F>(future: F) -> F::Output
    where
        F: std::future::Future,
    {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("build tokio runtime")
            .block_on(future)
    }

    fn empty_hook_metadata() -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::new()
    }

    fn build_loop_start_payload_input(
        loop_id: &str,
        ctx: &LoopContext,
        max_iterations: u32,
        iteration_current: u32,
        active_hat: Option<String>,
    ) -> HookPayloadBuilderInput {
        super::build_loop_start_payload_input(
            loop_id,
            ctx,
            max_iterations,
            iteration_current,
            active_hat,
            &empty_hook_metadata(),
        )
    }

    fn build_iteration_start_payload_input(
        loop_id: &str,
        ctx: &LoopContext,
        max_iterations: u32,
        iteration_current: u32,
        active_hat: Option<String>,
        selected_hat: Option<String>,
        selected_task: Option<String>,
    ) -> HookPayloadBuilderInput {
        super::build_iteration_start_payload_input(
            loop_id,
            ctx,
            max_iterations,
            iteration_current,
            active_hat,
            selected_hat,
            selected_task,
            &empty_hook_metadata(),
        )
    }

    fn build_plan_created_payload_input(
        loop_id: &str,
        ctx: &LoopContext,
        max_iterations: u32,
        iteration_current: u32,
        active_hat: Option<String>,
        selected_hat: Option<String>,
        selected_task: Option<String>,
    ) -> HookPayloadBuilderInput {
        super::build_plan_created_payload_input(
            loop_id,
            ctx,
            max_iterations,
            iteration_current,
            active_hat,
            selected_hat,
            selected_task,
            &empty_hook_metadata(),
        )
    }

    fn build_human_interact_payload_input(
        loop_id: &str,
        ctx: &LoopContext,
        max_iterations: u32,
        iteration_current: u32,
        active_hat: Option<String>,
        selected_hat: Option<String>,
        selected_task: Option<String>,
        human_interact: Option<serde_json::Value>,
    ) -> HookPayloadBuilderInput {
        super::build_human_interact_payload_input(
            loop_id,
            ctx,
            max_iterations,
            iteration_current,
            active_hat,
            selected_hat,
            selected_task,
            human_interact,
            &empty_hook_metadata(),
        )
    }

    fn build_loop_termination_payload_input(
        loop_id: &str,
        ctx: &LoopContext,
        max_iterations: u32,
        iteration_current: u32,
        active_hat: Option<String>,
        selected_hat: Option<String>,
        selected_task: Option<String>,
        termination_reason: &TerminationReason,
    ) -> HookPayloadBuilderInput {
        super::build_loop_termination_payload_input(
            loop_id,
            ctx,
            max_iterations,
            iteration_current,
            active_hat,
            selected_hat,
            selected_task,
            termination_reason,
            &empty_hook_metadata(),
        )
    }

    async fn dispatch_pre_loop_termination_hooks(
        event_loop: &EventLoop,
        hooks_dispatch_enabled: bool,
        loop_id: &str,
        hook_engine: &HookEngine,
        hook_executor: &HookExecutor,
        suspend_state_store: &SuspendStateStore,
        ctx: &LoopContext,
        max_iterations: u32,
        reason: TerminationReason,
    ) -> Result<TerminationReason> {
        let mut accumulated_hook_metadata = serde_json::Map::new();
        super::dispatch_pre_loop_termination_hooks(
            event_loop,
            hooks_dispatch_enabled,
            loop_id,
            hook_engine,
            hook_executor,
            suspend_state_store,
            ctx,
            max_iterations,
            &mut accumulated_hook_metadata,
            reason,
        )
        .await
    }

    async fn dispatch_post_loop_termination_hooks(
        event_loop: &EventLoop,
        hooks_dispatch_enabled: bool,
        loop_id: &str,
        hook_engine: &HookEngine,
        hook_executor: &HookExecutor,
        suspend_state_store: &SuspendStateStore,
        ctx: &LoopContext,
        max_iterations: u32,
        reason: TerminationReason,
    ) -> Result<TerminationReason> {
        let mut accumulated_hook_metadata = serde_json::Map::new();
        super::dispatch_post_loop_termination_hooks(
            event_loop,
            hooks_dispatch_enabled,
            loop_id,
            hook_engine,
            hook_executor,
            suspend_state_store,
            ctx,
            max_iterations,
            &mut accumulated_hook_metadata,
            reason,
        )
        .await
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_routes_by_phase_and_preserves_order() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let log_path = temp_dir.path().join("hook-dispatch.log");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreIterationStart,
            vec![
                recording_hook("pre-iteration-first", &log_path),
                recording_hook("pre-iteration-second", &log_path),
            ],
        );
        events.insert(
            HookPhaseEvent::PostLoopStart,
            vec![recording_hook("post-loop-only", &log_path)],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("ralph".to_string()),
                Some("builder".to_string()),
                Some("task-123".to_string()),
            ),
        );

        assert_eq!(
            read_hook_log(&log_path),
            vec![
                "pre-iteration-first|pre.iteration.start".to_string(),
                "pre-iteration-second|pre.iteration.start".to_string(),
            ]
        );

        dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostLoopStart,
            build_loop_start_payload_input("loop-test", &loop_ctx, 5, 1, Some("ralph".to_string())),
        );

        assert_eq!(
            read_hook_log(&log_path),
            vec![
                "pre-iteration-first|pre.iteration.start".to_string(),
                "pre-iteration-second|pre.iteration.start".to_string(),
                "post-loop-only|post.loop.start".to_string(),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ac13_mutation_disabled_json_output_is_inert_for_accumulator_and_downstream_payloads() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let payload_log_path = temp_dir
            .path()
            .join("hook-metadata-disabled-payloads.jsonl");

        let mut disabled_mutation_spec = hook_spec_with_command(
            "metadata-emitter",
            vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf '%s' '{\"metadata\":{\"risk_score\":0.72}}'".to_string(),
            ],
        );
        disabled_mutation_spec.mutate = hook_mutation_config(false);

        let mut events = std::collections::HashMap::new();
        events.insert(HookPhaseEvent::PreLoopStart, vec![disabled_mutation_spec]);
        events.insert(
            HookPhaseEvent::PostLoopStart,
            vec![payload_recording_hook(
                "payload-recorder",
                &payload_log_path,
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());
        let mut accumulated_hook_metadata = serde_json::Map::new();
        accumulated_hook_metadata.insert("upstream".to_string(), serde_json::json!("preserved"));

        let pre_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreLoopStart,
            super::build_loop_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                0,
                Some("planner".to_string()),
                &accumulated_hook_metadata,
            ),
        );

        assert_eq!(pre_outcomes.len(), 1);
        assert_eq!(pre_outcomes[0].disposition, HookDisposition::Pass);
        assert_eq!(pre_outcomes[0].failure, None);
        assert_eq!(
            pre_outcomes[0].mutation_parse_outcome,
            HookMutationParseOutcome::Disabled
        );

        let metadata_before_merge = accumulated_hook_metadata.clone();
        merge_accumulated_hook_metadata_from_outcomes(
            &mut accumulated_hook_metadata,
            &pre_outcomes,
        );
        assert_eq!(accumulated_hook_metadata, metadata_before_merge);

        let post_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostLoopStart,
            super::build_loop_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                0,
                Some("planner".to_string()),
                &accumulated_hook_metadata,
            ),
        );
        merge_accumulated_hook_metadata_from_outcomes(
            &mut accumulated_hook_metadata,
            &post_outcomes,
        );

        let payloads = read_hook_payload_log(&payload_log_path);
        assert_eq!(payloads.len(), 1);
        assert_eq!(
            payloads[0]["metadata"]["accumulated"],
            serde_json::json!({"upstream":"preserved"})
        );

        let payload_accumulated = payloads[0]["metadata"]["accumulated"]
            .as_object()
            .expect("metadata.accumulated object");
        assert!(!payload_accumulated.contains_key("hook_metadata"));
    }

    #[cfg(unix)]
    #[test]
    fn test_ac14_mutation_enabled_updates_only_namespaced_metadata_in_downstream_payloads() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let payload_log_path = temp_dir.path().join("hook-metadata-enabled-payloads.jsonl");

        let mut mutation_spec = hook_spec_with_command(
            "metadata-emitter",
            vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf '%s' '{\"metadata\":{\"risk_score\":0.72,\"gates\":[\"policy_check\"]}}'"
                    .to_string(),
            ],
        );
        mutation_spec.mutate = hook_mutation_config(true);

        let mut events = std::collections::HashMap::new();
        events.insert(HookPhaseEvent::PreLoopStart, vec![mutation_spec]);
        events.insert(
            HookPhaseEvent::PostLoopStart,
            vec![payload_recording_hook(
                "payload-recorder",
                &payload_log_path,
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());
        let mut accumulated_hook_metadata = serde_json::Map::new();
        accumulated_hook_metadata.insert("upstream".to_string(), serde_json::json!("preserved"));

        let pre_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreLoopStart,
            super::build_loop_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                0,
                Some("planner".to_string()),
                &accumulated_hook_metadata,
            ),
        );
        assert!(matches!(
            pre_outcomes[0].mutation_parse_outcome,
            HookMutationParseOutcome::Parsed { .. }
        ));

        merge_accumulated_hook_metadata_from_outcomes(
            &mut accumulated_hook_metadata,
            &pre_outcomes,
        );
        assert_eq!(
            serde_json::Value::Object(accumulated_hook_metadata.clone()),
            serde_json::json!({
                "upstream": "preserved",
                "hook_metadata": {
                    "metadata-emitter": {
                        "risk_score": 0.72,
                        "gates": ["policy_check"]
                    }
                }
            })
        );

        let post_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostLoopStart,
            super::build_loop_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                0,
                Some("planner".to_string()),
                &accumulated_hook_metadata,
            ),
        );
        merge_accumulated_hook_metadata_from_outcomes(
            &mut accumulated_hook_metadata,
            &post_outcomes,
        );

        let payloads = read_hook_payload_log(&payload_log_path);
        assert_eq!(payloads.len(), 1);
        let payload = &payloads[0];

        assert_eq!(payload["phase_event"], serde_json::json!("post.loop.start"));
        assert_eq!(
            payload["context"]["active_hat"],
            serde_json::json!("planner")
        );
        assert_eq!(
            payload["metadata"]["accumulated"],
            serde_json::json!({
                "upstream": "preserved",
                "hook_metadata": {
                    "metadata-emitter": {
                        "risk_score": 0.72,
                        "gates": ["policy_check"]
                    }
                }
            })
        );

        let payload_object = payload.as_object().expect("payload object");
        assert!(!payload_object.contains_key("prompt"));
        assert!(!payload_object.contains_key("events"));
        assert!(!payload_object.contains_key("config"));

        let context = payload["context"]
            .as_object()
            .expect("payload context object");
        assert!(!context.contains_key("prompt"));
        assert!(!context.contains_key("events"));
        assert!(!context.contains_key("config"));

        let payload_accumulated = payload["metadata"]["accumulated"]
            .as_object()
            .expect("metadata.accumulated object");
        assert!(!payload_accumulated.contains_key("risk_score"));
        assert!(!payload_accumulated.contains_key("gates"));
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_noop_when_disabled_or_unconfigured() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let log_path = temp_dir.path().join("hook-noop.log");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreIterationStart,
            vec![recording_hook("should-not-run", &log_path)],
        );

        let hook_engine = hook_engine_with_events(events);
        let empty_engine = hook_engine_with_events(std::collections::HashMap::new());
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let disabled_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            false,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("ralph".to_string()),
                Some("builder".to_string()),
                Some("task-123".to_string()),
            ),
        );

        let empty_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &empty_engine,
            &hook_executor,
            HookPhaseEvent::PreIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("ralph".to_string()),
                Some("builder".to_string()),
                Some("task-123".to_string()),
            ),
        );

        let mismatched_phase_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostLoopStart,
            build_loop_start_payload_input("loop-test", &loop_ctx, 5, 1, Some("ralph".to_string())),
        );

        assert!(
            disabled_outcomes.is_empty(),
            "disabled hooks must be a no-op"
        );
        assert!(
            empty_outcomes.is_empty(),
            "empty hooks config must be a no-op"
        );
        assert!(
            mismatched_phase_outcomes.is_empty(),
            "dispatching a phase without hooks must be a no-op"
        );
        assert!(
            !log_path.exists(),
            "hook log should not be created on no-op paths"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_returns_dispositions_and_failure_context() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreLoopStart,
            vec![
                hook_spec_with_command(
                    "hook-pass",
                    vec!["sh".to_string(), "-c".to_string(), "exit 0".to_string()],
                ),
                hook_spec_with_command(
                    "hook-warn",
                    vec!["sh".to_string(), "-c".to_string(), "exit 7".to_string()],
                ),
                hook_spec_with_command_and_on_error(
                    "hook-block",
                    vec!["sh".to_string(), "-c".to_string(), "exit 23".to_string()],
                    HookOnError::Block,
                ),
            ],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreLoopStart,
            build_loop_start_payload_input("loop-test", &loop_ctx, 5, 1, Some("ralph".to_string())),
        );

        assert_eq!(outcomes.len(), 3);

        assert_eq!(outcomes[0].hook_name, "hook-pass");
        assert_eq!(outcomes[0].phase_event, HookPhaseEvent::PreLoopStart);
        assert_eq!(outcomes[0].disposition, HookDisposition::Pass);
        assert!(outcomes[0].failure.is_none());

        assert_eq!(outcomes[1].hook_name, "hook-warn");
        assert_eq!(outcomes[1].disposition, HookDisposition::Warn);
        assert_eq!(
            outcomes[1].failure,
            Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(7),
                timed_out: false,
            })
        );

        assert_eq!(outcomes[2].hook_name, "hook-block");
        assert_eq!(outcomes[2].disposition, HookDisposition::Block);
        assert_eq!(
            outcomes[2].failure,
            Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(23),
                timed_out: false,
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_maps_executor_failures_to_on_error_disposition() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreLoopStart,
            vec![
                hook_spec_with_command(
                    "warn-exec-error",
                    vec!["definitely-not-a-real-exec-warn".to_string()],
                ),
                hook_spec_with_command_and_on_error(
                    "block-exec-error",
                    vec!["definitely-not-a-real-exec-block".to_string()],
                    HookOnError::Block,
                ),
            ],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreLoopStart,
            build_loop_start_payload_input("loop-test", &loop_ctx, 5, 1, Some("ralph".to_string())),
        );

        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].hook_name, "warn-exec-error");
        assert_eq!(outcomes[0].disposition, HookDisposition::Warn);
        match &outcomes[0].failure {
            Some(HookDispatchFailure::HookExecutionError { message }) => {
                assert!(
                    message.contains("definitely-not-a-real-exec-warn"),
                    "executor failure context should include missing command"
                );
            }
            other => panic!("expected execution error failure context, got {other:?}"),
        }

        assert_eq!(outcomes[1].hook_name, "block-exec-error");
        assert_eq!(outcomes[1].disposition, HookDisposition::Block);
        match &outcomes[1].failure {
            Some(HookDispatchFailure::HookExecutionError { message }) => {
                assert!(
                    message.contains("definitely-not-a-real-exec-block"),
                    "executor failure context should include missing command"
                );
            }
            other => panic!("expected execution error failure context, got {other:?}"),
        }
    }

    // AC-15: JSON-only mutation format errors must flow through lifecycle on_error dispositions.
    #[cfg(unix)]
    #[test]
    fn test_ac15_dispatch_phase_event_hooks_non_json_mutation_warn_continues_through_block_gate() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let mut warn_hook = hook_spec_with_command_and_on_error(
            "warn-invalid-mutation",
            vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf '%s' 'oops'".to_string(),
            ],
            HookOnError::Warn,
        );
        warn_hook.mutate = hook_mutation_config(true);

        let mut events = std::collections::HashMap::new();
        events.insert(HookPhaseEvent::PreLoopStart, vec![warn_hook]);

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreLoopStart,
            build_loop_start_payload_input("loop-test", &loop_ctx, 5, 1, Some("ralph".to_string())),
        );

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].disposition, HookDisposition::Warn);
        assert!(matches!(
            outcomes[0].mutation_parse_outcome,
            HookMutationParseOutcome::Invalid(_)
        ));
        assert!(matches!(
            &outcomes[0].failure,
            Some(HookDispatchFailure::InvalidMutationOutput { message })
            if message.contains("not valid JSON")
        ));
        assert!(fail_if_blocking_loop_start_outcomes(&outcomes).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_ac15_dispatch_phase_event_hooks_non_json_mutation_block_surfaces_invalid_output_reason()
    {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let mut block_hook = hook_spec_with_command_and_on_error(
            "block-invalid-mutation",
            vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf '%s' 'oops'".to_string(),
            ],
            HookOnError::Block,
        );
        block_hook.mutate = hook_mutation_config(true);

        let mut events = std::collections::HashMap::new();
        events.insert(HookPhaseEvent::PreLoopStart, vec![block_hook]);

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreLoopStart,
            build_loop_start_payload_input("loop-test", &loop_ctx, 5, 1, Some("ralph".to_string())),
        );

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].disposition, HookDisposition::Block);
        assert!(matches!(
            &outcomes[0].failure,
            Some(HookDispatchFailure::InvalidMutationOutput { message })
            if message.contains("not valid JSON")
        ));

        let block_error = fail_if_blocking_loop_start_outcomes(&outcomes)
            .expect_err("block disposition should fail loop.start boundary");
        let block_message = block_error.to_string();
        assert!(block_message.contains("block-invalid-mutation"));
        assert!(block_message.contains("pre.loop.start"));
        assert!(block_message.contains("invalid mutation output"));
        assert!(block_message.contains("not valid JSON"));
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_runtime_failure_takes_precedence_over_mutation_parse_error()
    {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let mut block_hook = hook_spec_with_command_and_on_error(
            "block-runtime-failure",
            vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf '%s' 'oops'; exit 23".to_string(),
            ],
            HookOnError::Block,
        );
        block_hook.mutate = hook_mutation_config(true);

        let mut events = std::collections::HashMap::new();
        events.insert(HookPhaseEvent::PreLoopStart, vec![block_hook]);

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreLoopStart,
            build_loop_start_payload_input("loop-test", &loop_ctx, 5, 1, Some("ralph".to_string())),
        );

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].disposition, HookDisposition::Block);
        assert_eq!(
            outcomes[0].failure,
            Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(23),
                timed_out: false,
            })
        );
        assert!(matches!(
            outcomes[0].mutation_parse_outcome,
            HookMutationParseOutcome::Invalid(_)
        ));

        let block_error = fail_if_blocking_loop_start_outcomes(&outcomes)
            .expect_err("block disposition should fail loop.start boundary");
        let block_message = block_error.to_string();
        assert!(block_message.contains("hook exited with code 23"));
        assert!(!block_message.contains("invalid mutation output"));
    }

    #[cfg(unix)]
    #[test]
    fn test_ac15_dispatch_phase_event_hooks_non_json_mutation_suspend_uses_wait_for_resume_gate() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let mut suspend_hook = hook_spec_with_command_and_on_error(
            "suspend-invalid-mutation",
            vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf '%s' 'oops'".to_string(),
            ],
            HookOnError::Suspend,
        );
        suspend_hook.mutate = hook_mutation_config(true);

        let mut events = std::collections::HashMap::new();
        events.insert(HookPhaseEvent::PreIterationStart, vec![suspend_hook]);

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());
        let suspend_state_store = SuspendStateStore::new(temp_dir.path());

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("planner".to_string()),
                None,
                None,
            ),
        );

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].disposition, HookDisposition::Suspend);
        assert!(matches!(
            &outcomes[0].failure,
            Some(HookDispatchFailure::InvalidMutationOutput { message })
            if message.contains("not valid JSON")
        ));
        assert!(fail_if_blocking_iteration_start_outcomes(&outcomes).is_ok());

        let resume_store = suspend_state_store.clone();
        let resume_handle = std::thread::spawn(move || {
            let wait_started_at = std::time::Instant::now();
            while !resume_store.suspend_state_path().exists() {
                assert!(
                    wait_started_at.elapsed() < Duration::from_secs(2),
                    "suspend-state should be written before resume"
                );
                std::thread::sleep(Duration::from_millis(10));
            }

            let suspend_state = resume_store
                .read_suspend_state()
                .expect("read suspend-state")
                .expect("suspend-state should exist while waiting");
            assert!(suspend_state.reason.contains("invalid mutation output"));
            assert!(suspend_state.reason.contains("not valid JSON"));

            resume_store
                .write_resume_requested()
                .expect("write resume signal");
        });

        let wait_result = block_on_test_future(wait_for_resume_if_suspended(
            &outcomes,
            "loop-test",
            &suspend_state_store,
        ))
        .expect("wait helper should succeed");

        resume_handle
            .join()
            .expect("resume helper thread should not panic");

        assert_eq!(wait_result, None);
        assert!(
            suspend_state_store
                .read_suspend_state()
                .expect("read suspend-state after resume")
                .is_none(),
            "suspend-state should be cleared after resume"
        );
        assert!(
            !suspend_state_store.resume_requested_path().exists(),
            "resume-requested should be consumed after resume"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_loop_start_dispatch_warn_continues_and_block_aborts() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreLoopStart,
            vec![hook_spec_with_command_and_on_error(
                "warn-pre-loop-start",
                vec!["sh".to_string(), "-c".to_string(), "exit 17".to_string()],
                HookOnError::Warn,
            )],
        );
        events.insert(
            HookPhaseEvent::PostLoopStart,
            vec![hook_spec_with_command_and_on_error(
                "block-post-loop-start",
                vec!["sh".to_string(), "-c".to_string(), "exit 29".to_string()],
                HookOnError::Block,
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let pre_loop_start_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreLoopStart,
            build_loop_start_payload_input("loop-test", &loop_ctx, 5, 0, None),
        );

        assert_eq!(pre_loop_start_outcomes.len(), 1);
        assert_eq!(
            pre_loop_start_outcomes[0].disposition,
            HookDisposition::Warn
        );
        assert_eq!(
            pre_loop_start_outcomes[0].failure,
            Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(17),
                timed_out: false,
            })
        );
        assert!(
            fail_if_blocking_loop_start_outcomes(&pre_loop_start_outcomes).is_ok(),
            "warn disposition should continue across loop.start boundary"
        );

        let post_loop_start_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostLoopStart,
            build_loop_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                0,
                Some("planner".to_string()),
            ),
        );

        assert_eq!(post_loop_start_outcomes.len(), 1);
        assert_eq!(
            post_loop_start_outcomes[0].disposition,
            HookDisposition::Block
        );
        let post_loop_start_error = fail_if_blocking_loop_start_outcomes(&post_loop_start_outcomes)
            .expect_err("block disposition should abort loop.start boundary");
        let post_loop_start_message = post_loop_start_error.to_string();
        assert!(post_loop_start_message.contains("block-post-loop-start"));
        assert!(post_loop_start_message.contains("post.loop.start"));
        assert!(post_loop_start_message.contains("hook exited with code 29"));
    }

    #[cfg(unix)]
    #[test]
    fn test_iteration_start_dispatch_warn_continues_and_block_aborts() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreIterationStart,
            vec![hook_spec_with_command_and_on_error(
                "warn-pre-iteration-start",
                vec!["sh".to_string(), "-c".to_string(), "exit 19".to_string()],
                HookOnError::Warn,
            )],
        );
        events.insert(
            HookPhaseEvent::PostIterationStart,
            vec![hook_spec_with_command_and_on_error(
                "block-post-iteration-start",
                vec!["sh".to_string(), "-c".to_string(), "exit 31".to_string()],
                HookOnError::Block,
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let pre_iteration_start_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("planner".to_string()),
                None,
                None,
            ),
        );

        assert_eq!(pre_iteration_start_outcomes.len(), 1);
        assert_eq!(
            pre_iteration_start_outcomes[0].disposition,
            HookDisposition::Warn
        );
        assert_eq!(
            pre_iteration_start_outcomes[0].failure,
            Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(19),
                timed_out: false,
            })
        );
        assert!(
            fail_if_blocking_iteration_start_outcomes(&pre_iteration_start_outcomes).is_ok(),
            "warn disposition should continue across iteration.start boundary"
        );

        let post_iteration_start_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("planner".to_string()),
                Some("builder".to_string()),
                Some("task-123".to_string()),
            ),
        );

        assert_eq!(post_iteration_start_outcomes.len(), 1);
        assert_eq!(
            post_iteration_start_outcomes[0].disposition,
            HookDisposition::Block
        );
        let post_iteration_start_error =
            fail_if_blocking_iteration_start_outcomes(&post_iteration_start_outcomes)
                .expect_err("block disposition should abort iteration.start boundary");
        let post_iteration_start_message = post_iteration_start_error.to_string();
        assert!(post_iteration_start_message.contains("block-post-iteration-start"));
        assert!(post_iteration_start_message.contains("post.iteration.start"));
        assert!(post_iteration_start_message.contains("hook exited with code 31"));
    }

    #[cfg(unix)]
    #[test]
    fn test_plan_created_lifecycle_hooks_dispatch_only_for_semantic_plan_batches() {
        use std::io::Write;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let (mut event_loop, loop_ctx) = dispatch_test_event_loop_with_context(temp_dir.path());
        let events_path = loop_ctx.events_path();
        std::fs::create_dir_all(events_path.parent().expect("events path parent"))
            .expect("create events directory");

        let mut events_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)
            .expect("open events file");
        writeln!(
            events_file,
            r#"{{"topic":"task.start","payload":"noop","ts":"2024-01-01T00:00:00Z"}}"#
        )
        .expect("write non-plan event");
        events_file.flush().expect("flush non-plan event");

        let log_path = temp_dir.path().join("plan-created-hook-payloads.jsonl");
        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PrePlanCreated,
            vec![payload_recording_hook("pre-plan-created", &log_path)],
        );
        events.insert(
            HookPhaseEvent::PostPlanCreated,
            vec![payload_recording_hook("post-plan-created", &log_path)],
        );
        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();

        assert!(
            !event_loop
                .has_pending_plan_events_in_jsonl()
                .expect("peek non-plan events"),
            "non-plan batches must not trigger pre.plan.created"
        );

        let processed_non_plan = event_loop
            .process_events_from_jsonl()
            .expect("process non-plan batch");
        assert!(processed_non_plan.had_events);
        assert!(
            !processed_non_plan.had_plan_events,
            "non-plan batches must not trigger post.plan.created"
        );
        assert!(
            !log_path.exists(),
            "plan.created hooks should not run for non-plan batches"
        );

        writeln!(
            events_file,
            r#"{{"topic":"plan.created","payload":"ready","ts":"2024-01-01T00:00:01Z"}}"#
        )
        .expect("write plan event");
        events_file.flush().expect("flush plan event");

        assert!(
            event_loop
                .has_pending_plan_events_in_jsonl()
                .expect("peek plan events"),
            "plan.* batches should trigger pre.plan.created"
        );

        let pre_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PrePlanCreated,
            build_plan_created_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                event_loop.state().iteration,
                Some("planner".to_string()),
                Some("planner".to_string()),
                None,
            ),
        );
        assert!(fail_if_blocking_plan_created_outcomes(&pre_outcomes).is_ok());

        let processed_plan = event_loop
            .process_events_from_jsonl()
            .expect("process plan batch");
        assert!(processed_plan.had_events);
        assert!(
            processed_plan.had_plan_events,
            "plan.* batches should trigger post.plan.created"
        );

        let post_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostPlanCreated,
            build_plan_created_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                event_loop.state().iteration,
                Some("planner".to_string()),
                Some("planner".to_string()),
                None,
            ),
        );
        assert!(fail_if_blocking_plan_created_outcomes(&post_outcomes).is_ok());

        let payloads = read_hook_payload_log(&log_path);
        let observed_phases: Vec<&str> = payloads
            .iter()
            .map(|payload| {
                payload["phase_event"]
                    .as_str()
                    .expect("phase_event should be present")
            })
            .collect();

        assert_eq!(
            observed_phases,
            vec!["pre.plan.created", "post.plan.created"],
            "plan.created hooks should dispatch exactly once around semantic plan batches"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_human_interact_lifecycle_hooks_dispatch_with_post_outcome_context() {
        use std::io::Write;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let (mut event_loop, loop_ctx) = dispatch_test_event_loop_with_context(temp_dir.path());
        let events_path = loop_ctx.events_path();
        std::fs::create_dir_all(events_path.parent().expect("events path parent"))
            .expect("create events directory");

        let mut events_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)
            .expect("open events file");
        writeln!(
            events_file,
            r#"{{"topic":"human.interact","payload":"Need approval?","ts":"2024-01-01T00:00:00Z"}}"#
        )
        .expect("write human.interact event");
        events_file.flush().expect("flush human.interact event");

        let log_path = temp_dir.path().join("human-interact-hook-payloads.jsonl");
        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreHumanInteract,
            vec![payload_recording_hook("pre-human-interact", &log_path)],
        );
        events.insert(
            HookPhaseEvent::PostHumanInteract,
            vec![payload_recording_hook("post-human-interact", &log_path)],
        );
        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();

        let pending_context = event_loop
            .pending_human_interact_context_in_jsonl()
            .expect("peek pending human.interact context")
            .expect("pending human.interact context should exist");
        assert_eq!(
            pending_context["question"],
            serde_json::json!("Need approval?")
        );
        assert!(
            pending_context.get("outcome").is_none(),
            "pre human.interact context should not include an outcome"
        );

        let pre_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreHumanInteract,
            build_human_interact_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                event_loop.state().iteration,
                Some("planner".to_string()),
                Some("planner".to_string()),
                None,
                Some(pending_context),
            ),
        );
        assert!(fail_if_blocking_human_interact_outcomes(&pre_outcomes).is_ok());

        let processed = event_loop
            .process_events_from_jsonl()
            .expect("process human.interact batch");
        let post_context = processed
            .human_interact_context
            .expect("processed context should include human.interact outcome");
        assert_eq!(
            post_context["question"],
            serde_json::json!("Need approval?")
        );
        assert_eq!(
            post_context["outcome"],
            serde_json::json!("no_robot_service")
        );

        let post_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostHumanInteract,
            build_human_interact_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                event_loop.state().iteration,
                Some("planner".to_string()),
                Some("planner".to_string()),
                None,
                Some(post_context),
            ),
        );
        assert!(fail_if_blocking_human_interact_outcomes(&post_outcomes).is_ok());

        let payloads = read_hook_payload_log(&log_path);
        assert_eq!(payloads.len(), 2);
        assert_eq!(
            payloads[0]["phase_event"],
            serde_json::json!("pre.human.interact")
        );
        assert_eq!(
            payloads[0]["context"]["human_interact"]["question"],
            serde_json::json!("Need approval?")
        );
        assert!(
            payloads[0]["context"]["human_interact"]
                .get("outcome")
                .is_none(),
            "pre.human.interact payload should not include outcome"
        );

        assert_eq!(
            payloads[1]["phase_event"],
            serde_json::json!("post.human.interact")
        );
        assert_eq!(
            payloads[1]["context"]["human_interact"]["question"],
            serde_json::json!("Need approval?")
        );
        assert_eq!(
            payloads[1]["context"]["human_interact"]["outcome"],
            serde_json::json!("no_robot_service")
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_loop_termination_lifecycle_hooks_dispatch_complete_and_error_boundaries() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let log_path = temp_dir.path().join("loop-termination-hook-payloads.jsonl");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreLoopComplete,
            vec![payload_recording_hook("pre-loop-complete", &log_path)],
        );
        events.insert(
            HookPhaseEvent::PostLoopComplete,
            vec![payload_recording_hook("post-loop-complete", &log_path)],
        );
        events.insert(
            HookPhaseEvent::PreLoopError,
            vec![payload_recording_hook("pre-loop-error", &log_path)],
        );
        events.insert(
            HookPhaseEvent::PostLoopError,
            vec![payload_recording_hook("post-loop-error", &log_path)],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());
        let suspend_state_store = SuspendStateStore::new(temp_dir.path());

        let completed_reason = block_on_test_future(dispatch_pre_loop_termination_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            &suspend_state_store,
            &loop_ctx,
            5,
            TerminationReason::CompletionPromise,
        ))
        .expect("pre.loop.complete dispatch should succeed");
        let completed_reason = block_on_test_future(dispatch_post_loop_termination_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            &suspend_state_store,
            &loop_ctx,
            5,
            completed_reason,
        ))
        .expect("post.loop.complete dispatch should succeed");
        assert_eq!(completed_reason, TerminationReason::CompletionPromise);

        let error_reason = block_on_test_future(dispatch_pre_loop_termination_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            &suspend_state_store,
            &loop_ctx,
            5,
            TerminationReason::MaxRuntime,
        ))
        .expect("pre.loop.error dispatch should succeed");
        let error_reason = block_on_test_future(dispatch_post_loop_termination_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            &suspend_state_store,
            &loop_ctx,
            5,
            error_reason,
        ))
        .expect("post.loop.error dispatch should succeed");
        assert_eq!(error_reason, TerminationReason::MaxRuntime);

        let payloads = read_hook_payload_log(&log_path);
        let phases: Vec<&str> = payloads
            .iter()
            .map(|payload| {
                payload["phase_event"]
                    .as_str()
                    .expect("phase_event should be present")
            })
            .collect();
        let reasons: Vec<&str> = payloads
            .iter()
            .map(|payload| {
                payload["context"]["termination_reason"]
                    .as_str()
                    .expect("termination_reason should be present")
            })
            .collect();

        assert_eq!(
            phases,
            vec![
                "pre.loop.complete",
                "post.loop.complete",
                "pre.loop.error",
                "post.loop.error"
            ]
        );
        assert_eq!(
            reasons,
            vec!["completed", "completed", "max_runtime", "max_runtime"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_iteration_start_suspend_waits_for_resume_and_clears_artifacts_before_continuing() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreIterationStart,
            vec![hook_spec_with_command_and_on_error(
                "suspend-pre-iteration-start",
                vec!["sh".to_string(), "-c".to_string(), "exit 41".to_string()],
                HookOnError::Suspend,
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let pre_iteration_start_outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("planner".to_string()),
                None,
                None,
            ),
        );

        assert_eq!(pre_iteration_start_outcomes.len(), 1);
        assert_eq!(
            pre_iteration_start_outcomes[0].disposition,
            HookDisposition::Suspend
        );
        assert_eq!(
            pre_iteration_start_outcomes[0].failure,
            Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(41),
                timed_out: false,
            })
        );
        assert!(
            fail_if_blocking_iteration_start_outcomes(&pre_iteration_start_outcomes).is_ok(),
            "suspend disposition should not block iteration.start boundary"
        );

        let suspend_state_store = SuspendStateStore::new(temp_dir.path());

        let wait_result = block_on_test_future(async {
            let wait_outcomes = pre_iteration_start_outcomes.clone();
            let wait_store = suspend_state_store.clone();
            let wait_handle = tokio::spawn(async move {
                wait_for_resume_if_suspended(&wait_outcomes, "loop-test", &wait_store).await
            });

            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if suspend_state_store.suspend_state_path().exists() {
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
            })
            .await
            .expect("suspend-state should be written before resume");

            let suspend_state = suspend_state_store
                .read_suspend_state()
                .expect("read suspend-state")
                .expect("suspend-state should exist while waiting for resume");

            assert_eq!(suspend_state.loop_id, "loop-test");
            assert_eq!(suspend_state.phase_event, HookPhaseEvent::PreIterationStart);
            assert_eq!(suspend_state.hook_name, "suspend-pre-iteration-start");
            assert_eq!(suspend_state.suspend_mode, HookSuspendMode::WaitForResume);
            assert!(!suspend_state_store.resume_requested_path().exists());

            suspend_state_store
                .write_resume_requested()
                .expect("write resume signal");

            tokio::time::timeout(Duration::from_secs(2), wait_handle)
                .await
                .expect("wait_for_resume helper should complete after resume signal")
                .expect("wait_for_resume task should not panic")
        })
        .expect("wait helper should succeed");

        assert_eq!(wait_result, None);
        assert!(
            suspend_state_store
                .read_suspend_state()
                .expect("read suspend-state after resume")
                .is_none(),
            "suspend-state should be cleared after resume"
        );
        assert!(
            !suspend_state_store.resume_requested_path().exists(),
            "resume-requested should be consumed after resume"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_retry_backoff_recovers_before_exhaustion() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let attempts_path = temp_dir.path().join("retry-backoff-attempts.txt");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreIterationStart,
            vec![hook_spec_with_command_and_on_error_and_suspend_mode(
                "retry-backoff-pre-iteration-start",
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    r#"attempts_file="$1"
attempt=0
if [ -f "$attempts_file" ]; then
  attempt="$(cat "$attempts_file")"
fi
attempt=$((attempt + 1))
printf '%s' "$attempt" > "$attempts_file"
if [ "$attempt" -lt 3 ]; then
  exit 41
fi
exit 0"#
                        .to_string(),
                    "retry-backoff-hook".to_string(),
                    attempts_path.to_string_lossy().into_owned(),
                ],
                HookOnError::Suspend,
                Some(HookSuspendMode::RetryBackoff),
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop_with_diagnostics(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("planner".to_string()),
                None,
                None,
            ),
        );

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].disposition, HookDisposition::Pass);
        assert_eq!(outcomes[0].suspend_mode, HookSuspendMode::RetryBackoff);
        assert_eq!(outcomes[0].failure, None);

        let attempts = std::fs::read_to_string(&attempts_path).expect("read attempts");
        assert_eq!(attempts.trim(), "3", "hook should recover on third attempt");

        let telemetry_entries = read_hook_run_telemetry_entries(temp_dir.path());
        assert_eq!(telemetry_entries.len(), 3);
        assert_eq!(
            telemetry_entries
                .iter()
                .map(|entry| entry.retry_attempt)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(
            telemetry_entries
                .iter()
                .all(|entry| entry.retry_max_attempts == 4)
        );
        assert!(
            telemetry_entries
                .iter()
                .all(|entry| entry.suspend_mode == HookSuspendMode::RetryBackoff)
        );
        assert_eq!(
            telemetry_entries
                .iter()
                .map(|entry| entry.disposition)
                .collect::<Vec<_>>(),
            vec![
                HookDisposition::Suspend,
                HookDisposition::Suspend,
                HookDisposition::Pass,
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_retry_backoff_exhausts_to_suspend() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let attempts_path = temp_dir.path().join("retry-backoff-attempts.txt");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PostIterationStart,
            vec![hook_spec_with_command_and_on_error_and_suspend_mode(
                "retry-backoff-post-iteration-start",
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    r#"attempts_file="$1"
attempt=0
if [ -f "$attempts_file" ]; then
  attempt="$(cat "$attempts_file")"
fi
attempt=$((attempt + 1))
printf '%s' "$attempt" > "$attempts_file"
exit 51"#
                        .to_string(),
                    "retry-backoff-hook".to_string(),
                    attempts_path.to_string_lossy().into_owned(),
                ],
                HookOnError::Suspend,
                Some(HookSuspendMode::RetryBackoff),
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("planner".to_string()),
                Some("builder".to_string()),
                Some("task-123".to_string()),
            ),
        );

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].disposition, HookDisposition::Suspend);
        assert_eq!(outcomes[0].suspend_mode, HookSuspendMode::RetryBackoff);
        assert_eq!(
            outcomes[0].failure,
            Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(51),
                timed_out: false,
            })
        );

        let attempts: usize = std::fs::read_to_string(&attempts_path)
            .expect("read attempts")
            .trim()
            .parse()
            .expect("parse attempts");
        assert_eq!(
            attempts,
            RETRY_BACKOFF_DELAYS_MS.len() + 1,
            "retry_backoff should cap retries at the configured schedule"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_retry_backoff_yields_to_stop_signal() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let attempts_path = temp_dir.path().join("retry-backoff-attempts.txt");
        std::fs::create_dir_all(temp_dir.path().join(".ralph")).expect("create .ralph");
        std::fs::write(temp_dir.path().join(".ralph/stop-requested"), "")
            .expect("write stop signal");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreLoopStart,
            vec![hook_spec_with_command_and_on_error_and_suspend_mode(
                "retry-backoff-pre-loop-start",
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    r#"attempts_file="$1"
attempt=0
if [ -f "$attempts_file" ]; then
  attempt="$(cat "$attempts_file")"
fi
attempt=$((attempt + 1))
printf '%s' "$attempt" > "$attempts_file"
exit 61"#
                        .to_string(),
                    "retry-backoff-hook".to_string(),
                    attempts_path.to_string_lossy().into_owned(),
                ],
                HookOnError::Suspend,
                Some(HookSuspendMode::RetryBackoff),
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreLoopStart,
            build_loop_start_payload_input("loop-test", &loop_ctx, 5, 1, Some("ralph".to_string())),
        );

        let attempts = std::fs::read_to_string(&attempts_path).expect("read attempts");
        assert_eq!(
            attempts.trim(),
            "1",
            "stop signal should short-circuit retry_backoff retries"
        );

        let suspend_state_store = SuspendStateStore::new(temp_dir.path());
        let wait_result = block_on_test_future(wait_for_resume_if_suspended(
            &outcomes,
            "loop-test",
            &suspend_state_store,
        ))
        .expect("wait helper should succeed");

        assert_eq!(wait_result, Some(TerminationReason::Stopped));
        assert!(!temp_dir.path().join(".ralph/stop-requested").exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_wait_then_retry_recovers_after_resume() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let attempts_path = temp_dir.path().join("wait-then-retry-attempts.txt");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreIterationStart,
            vec![hook_spec_with_command_and_on_error_and_suspend_mode(
                "wait-then-retry-pre-iteration-start",
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    r#"attempts_file="$1"
attempt=0
if [ -f "$attempts_file" ]; then
  attempt="$(cat "$attempts_file")"
fi
attempt=$((attempt + 1))
printf '%s' "$attempt" > "$attempts_file"
if [ "$attempt" -lt 2 ]; then
  exit 71
fi
exit 0"#
                        .to_string(),
                    "wait-then-retry-hook".to_string(),
                    attempts_path.to_string_lossy().into_owned(),
                ],
                HookOnError::Suspend,
                Some(HookSuspendMode::WaitThenRetry),
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop_with_diagnostics(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());
        let suspend_state_store = SuspendStateStore::new(temp_dir.path());

        let resume_store = suspend_state_store.clone();
        let resume_handle = std::thread::spawn(move || {
            let wait_started_at = std::time::Instant::now();
            while !resume_store.suspend_state_path().exists() {
                assert!(
                    wait_started_at.elapsed() < Duration::from_secs(2),
                    "wait_then_retry should persist suspend-state before waiting"
                );
                std::thread::sleep(Duration::from_millis(10));
            }

            resume_store
                .write_resume_requested()
                .expect("write resume signal");
        });

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("planner".to_string()),
                None,
                None,
            ),
        );

        resume_handle
            .join()
            .expect("resume helper thread should not panic");

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].disposition, HookDisposition::Pass);
        assert_eq!(outcomes[0].suspend_mode, HookSuspendMode::WaitThenRetry);
        assert_eq!(outcomes[0].failure, None);

        let attempts = std::fs::read_to_string(&attempts_path).expect("read attempts");
        assert_eq!(
            attempts.trim(),
            "2",
            "wait_then_retry should run exactly one retry after resume"
        );
        assert!(
            suspend_state_store
                .read_suspend_state()
                .expect("read suspend-state after wait_then_retry")
                .is_none(),
            "suspend-state should be cleared after wait_then_retry resume"
        );
        assert!(
            !suspend_state_store.resume_requested_path().exists(),
            "resume signal should be consumed under wait_then_retry"
        );

        let telemetry_entries = read_hook_run_telemetry_entries(temp_dir.path());
        assert_eq!(telemetry_entries.len(), 2);
        assert_eq!(
            telemetry_entries
                .iter()
                .map(|entry| entry.retry_attempt)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(
            telemetry_entries
                .iter()
                .all(|entry| entry.retry_max_attempts == 2)
        );
        assert!(
            telemetry_entries
                .iter()
                .all(|entry| entry.suspend_mode == HookSuspendMode::WaitThenRetry)
        );
        assert_eq!(
            telemetry_entries
                .iter()
                .map(|entry| entry.disposition)
                .collect::<Vec<_>>(),
            vec![HookDisposition::Suspend, HookDisposition::Pass]
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_wait_then_retry_retry_failure_remains_suspended() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let attempts_path = temp_dir.path().join("wait-then-retry-attempts.txt");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PostIterationStart,
            vec![hook_spec_with_command_and_on_error_and_suspend_mode(
                "wait-then-retry-post-iteration-start",
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    r#"attempts_file="$1"
attempt=0
if [ -f "$attempts_file" ]; then
  attempt="$(cat "$attempts_file")"
fi
attempt=$((attempt + 1))
printf '%s' "$attempt" > "$attempts_file"
exit 72"#
                        .to_string(),
                    "wait-then-retry-hook".to_string(),
                    attempts_path.to_string_lossy().into_owned(),
                ],
                HookOnError::Suspend,
                Some(HookSuspendMode::WaitThenRetry),
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());
        let suspend_state_store = SuspendStateStore::new(temp_dir.path());

        let resume_store = suspend_state_store.clone();
        let resume_handle = std::thread::spawn(move || {
            let wait_started_at = std::time::Instant::now();
            while !resume_store.suspend_state_path().exists() {
                assert!(
                    wait_started_at.elapsed() < Duration::from_secs(2),
                    "wait_then_retry should persist suspend-state before waiting"
                );
                std::thread::sleep(Duration::from_millis(10));
            }

            resume_store
                .write_resume_requested()
                .expect("write resume signal");
        });

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PostIterationStart,
            build_iteration_start_payload_input(
                "loop-test",
                &loop_ctx,
                5,
                1,
                Some("planner".to_string()),
                Some("builder".to_string()),
                Some("task-123".to_string()),
            ),
        );

        resume_handle
            .join()
            .expect("resume helper thread should not panic");

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].disposition, HookDisposition::Suspend);
        assert_eq!(outcomes[0].suspend_mode, HookSuspendMode::WaitThenRetry);
        assert_eq!(
            outcomes[0].failure,
            Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(72),
                timed_out: false,
            })
        );

        let attempts = std::fs::read_to_string(&attempts_path).expect("read attempts");
        assert_eq!(
            attempts.trim(),
            "2",
            "wait_then_retry should run a single retry attempt after resume"
        );
        assert!(
            suspend_state_store
                .read_suspend_state()
                .expect("read suspend-state after wait_then_retry")
                .is_none(),
            "first wait_then_retry suspend-state should be cleared after resume"
        );
        assert!(
            !suspend_state_store.resume_requested_path().exists(),
            "resume signal should be consumed after wait_then_retry"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_dispatch_phase_event_hooks_wait_then_retry_prioritizes_stop_over_resume() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let attempts_path = temp_dir.path().join("wait-then-retry-attempts.txt");
        std::fs::create_dir_all(temp_dir.path().join(".ralph")).expect("create .ralph");
        std::fs::write(temp_dir.path().join(".ralph/stop-requested"), "")
            .expect("write stop signal");
        std::fs::write(temp_dir.path().join(".ralph/resume-requested"), "")
            .expect("write resume signal");

        let mut events = std::collections::HashMap::new();
        events.insert(
            HookPhaseEvent::PreLoopStart,
            vec![hook_spec_with_command_and_on_error_and_suspend_mode(
                "wait-then-retry-pre-loop-start",
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    r#"attempts_file="$1"
attempt=0
if [ -f "$attempts_file" ]; then
  attempt="$(cat "$attempts_file")"
fi
attempt=$((attempt + 1))
printf '%s' "$attempt" > "$attempts_file"
exit 73"#
                        .to_string(),
                    "wait-then-retry-hook".to_string(),
                    attempts_path.to_string_lossy().into_owned(),
                ],
                HookOnError::Suspend,
                Some(HookSuspendMode::WaitThenRetry),
            )],
        );

        let hook_engine = hook_engine_with_events(events);
        let hook_executor = HookExecutor::new();
        let event_loop = dispatch_test_event_loop(temp_dir.path());
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());
        let suspend_state_store = SuspendStateStore::new(temp_dir.path());

        let outcomes = dispatch_phase_event_hooks(
            &event_loop,
            true,
            "loop-test",
            &hook_engine,
            &hook_executor,
            HookPhaseEvent::PreLoopStart,
            build_loop_start_payload_input("loop-test", &loop_ctx, 5, 1, Some("ralph".to_string())),
        );

        let attempts = std::fs::read_to_string(&attempts_path).expect("read attempts");
        assert_eq!(
            attempts.trim(),
            "1",
            "stop signal should prevent wait_then_retry from running the retry"
        );

        let wait_result = block_on_test_future(wait_for_resume_if_suspended(
            &outcomes,
            "loop-test",
            &suspend_state_store,
        ))
        .expect("wait helper should succeed");

        assert_eq!(wait_result, Some(TerminationReason::Stopped));
        assert!(!temp_dir.path().join(".ralph/stop-requested").exists());
        assert!(!suspend_state_store.resume_requested_path().exists());
    }

    #[test]
    fn test_run_retry_backoff_policy_replays_configured_schedule_deterministically() {
        let mut observed_delays_ms = Vec::new();
        let mut observed_retry_attempts = Vec::new();

        let outcome = run_retry_backoff_policy(
            "pre.iteration.start",
            "retry-hook",
            &[3, 5, 8],
            |delay, retry_attempt| {
                observed_delays_ms.push(delay.as_millis() as u64);
                assert_eq!(retry_attempt, observed_delays_ms.len());
                RetryBackoffDelayOutcome::Elapsed
            },
            |retry_attempt| {
                observed_retry_attempts.push(retry_attempt);
                if retry_attempt == 4 {
                    HookDispatchOutcome {
                        phase_event: HookPhaseEvent::PreIterationStart,
                        hook_name: "retry-hook".to_string(),
                        disposition: HookDisposition::Pass,
                        suspend_mode: HookSuspendMode::RetryBackoff,
                        failure: None,

                        mutation_parse_outcome: HookMutationParseOutcome::Disabled,
                    }
                } else {
                    suspend_outcome_with_mode(
                        HookPhaseEvent::PreIterationStart,
                        "retry-hook",
                        HookSuspendMode::RetryBackoff,
                    )
                }
            },
            suspend_outcome_with_mode(
                HookPhaseEvent::PreIterationStart,
                "retry-hook",
                HookSuspendMode::RetryBackoff,
            ),
        );

        assert_eq!(observed_delays_ms, vec![3, 5, 8]);
        assert_eq!(observed_retry_attempts, vec![2, 3, 4]);
        assert_eq!(outcome.disposition, HookDisposition::Pass);
        assert_eq!(outcome.failure, None);
    }

    #[test]
    fn test_run_retry_backoff_policy_exhausts_after_last_configured_delay() {
        let mut observed_retry_attempts = Vec::new();

        let outcome = run_retry_backoff_policy(
            "post.iteration.start",
            "retry-hook",
            &[11, 13],
            |_delay, _retry_attempt| RetryBackoffDelayOutcome::Elapsed,
            |retry_attempt| {
                observed_retry_attempts.push(retry_attempt);
                suspend_outcome_with_mode(
                    HookPhaseEvent::PostIterationStart,
                    "retry-hook",
                    HookSuspendMode::RetryBackoff,
                )
            },
            suspend_outcome_with_mode(
                HookPhaseEvent::PostIterationStart,
                "retry-hook",
                HookSuspendMode::RetryBackoff,
            ),
        );

        assert_eq!(observed_retry_attempts, vec![2, 3]);
        assert_eq!(outcome.disposition, HookDisposition::Suspend);
        assert_eq!(
            outcome.failure,
            Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(41),
                timed_out: false,
            })
        );
    }

    #[test]
    fn test_run_retry_backoff_policy_stop_signal_short_circuits_before_retry_attempt() {
        let initial_outcome = suspend_outcome_with_mode(
            HookPhaseEvent::PreLoopStart,
            "retry-hook",
            HookSuspendMode::RetryBackoff,
        );
        let mut retry_attempt_called = false;

        let outcome = run_retry_backoff_policy(
            "pre.loop.start",
            "retry-hook",
            &[21, 34],
            |_delay, _retry_attempt| RetryBackoffDelayOutcome::StopRequested,
            |_retry_attempt| {
                retry_attempt_called = true;
                initial_outcome.clone()
            },
            initial_outcome.clone(),
        );

        assert!(!retry_attempt_called);
        assert_eq!(outcome, initial_outcome);
    }

    #[test]
    fn test_run_wait_then_retry_policy_resume_retries_once_and_returns_retry_result() {
        let mut clear_suspend_calls = 0usize;
        let mut retry_calls = 0usize;

        let outcome = run_wait_then_retry_policy(
            "pre.iteration.start",
            "wait-hook",
            || Ok(SuspendWaitOutcome::Resume),
            || {
                clear_suspend_calls += 1;
                Ok(())
            },
            || {
                retry_calls += 1;
                HookDispatchOutcome {
                    phase_event: HookPhaseEvent::PreIterationStart,
                    hook_name: "wait-hook".to_string(),
                    disposition: HookDisposition::Pass,
                    suspend_mode: HookSuspendMode::WaitThenRetry,
                    failure: None,

                    mutation_parse_outcome: HookMutationParseOutcome::Disabled,
                }
            },
            suspend_outcome_with_mode(
                HookPhaseEvent::PreIterationStart,
                "wait-hook",
                HookSuspendMode::WaitThenRetry,
            ),
        );

        assert_eq!(clear_suspend_calls, 1);
        assert_eq!(retry_calls, 1);
        assert_eq!(outcome.disposition, HookDisposition::Pass);
        assert_eq!(outcome.failure, None);
    }

    #[test]
    fn test_run_wait_then_retry_policy_retry_failure_returns_suspend() {
        let mut clear_suspend_calls = 0usize;
        let mut retry_calls = 0usize;

        let outcome = run_wait_then_retry_policy(
            "post.iteration.start",
            "wait-hook",
            || Ok(SuspendWaitOutcome::Resume),
            || {
                clear_suspend_calls += 1;
                Ok(())
            },
            || {
                retry_calls += 1;
                suspend_outcome_with_mode(
                    HookPhaseEvent::PostIterationStart,
                    "wait-hook",
                    HookSuspendMode::WaitThenRetry,
                )
            },
            suspend_outcome_with_mode(
                HookPhaseEvent::PostIterationStart,
                "wait-hook",
                HookSuspendMode::WaitThenRetry,
            ),
        );

        assert_eq!(clear_suspend_calls, 1);
        assert_eq!(retry_calls, 1);
        assert_eq!(outcome.disposition, HookDisposition::Suspend);
        assert_eq!(
            outcome.failure,
            Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(41),
                timed_out: false,
            })
        );
    }

    #[test]
    fn test_run_wait_then_retry_policy_stop_skips_retry_path() {
        let initial_outcome = suspend_outcome_with_mode(
            HookPhaseEvent::PreLoopStart,
            "wait-hook",
            HookSuspendMode::WaitThenRetry,
        );
        let mut clear_suspend_called = false;
        let mut retry_called = false;

        let outcome = run_wait_then_retry_policy(
            "pre.loop.start",
            "wait-hook",
            || Ok(SuspendWaitOutcome::Stop),
            || {
                clear_suspend_called = true;
                Ok(())
            },
            || {
                retry_called = true;
                HookDispatchOutcome {
                    phase_event: HookPhaseEvent::PreLoopStart,
                    hook_name: "wait-hook".to_string(),
                    disposition: HookDisposition::Pass,
                    suspend_mode: HookSuspendMode::WaitThenRetry,
                    failure: None,

                    mutation_parse_outcome: HookMutationParseOutcome::Disabled,
                }
            },
            initial_outcome.clone(),
        );

        assert!(!clear_suspend_called);
        assert!(!retry_called);
        assert_eq!(outcome, initial_outcome);
    }

    #[test]
    fn test_fail_if_blocking_loop_start_outcomes_allows_non_blocking_dispositions() {
        let outcomes = vec![
            HookDispatchOutcome {
                phase_event: HookPhaseEvent::PreLoopStart,
                hook_name: "warn-hook".to_string(),
                disposition: HookDisposition::Warn,
                suspend_mode: HookSuspendMode::WaitForResume,
                failure: Some(HookDispatchFailure::HookRunFailed {
                    exit_code: Some(7),
                    timed_out: false,
                }),

                mutation_parse_outcome: HookMutationParseOutcome::Disabled,
            },
            HookDispatchOutcome {
                phase_event: HookPhaseEvent::PostLoopStart,
                hook_name: "pass-hook".to_string(),
                disposition: HookDisposition::Pass,
                suspend_mode: HookSuspendMode::WaitForResume,
                failure: None,

                mutation_parse_outcome: HookMutationParseOutcome::Disabled,
            },
        ];

        assert!(fail_if_blocking_loop_start_outcomes(&outcomes).is_ok());
    }

    #[test]
    fn test_fail_if_blocking_loop_start_outcomes_surfaces_failure_context() {
        let blocked_exit_outcomes = vec![HookDispatchOutcome {
            phase_event: HookPhaseEvent::PostLoopStart,
            hook_name: "block-hook".to_string(),
            disposition: HookDisposition::Block,
            suspend_mode: HookSuspendMode::WaitForResume,
            failure: Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(42),
                timed_out: false,
            }),

            mutation_parse_outcome: HookMutationParseOutcome::Disabled,
        }];

        let blocked_exit_error = fail_if_blocking_loop_start_outcomes(&blocked_exit_outcomes)
            .expect_err("block disposition should fail loop.start boundary");
        let blocked_exit_message = blocked_exit_error.to_string();
        assert!(blocked_exit_message.contains("block-hook"));
        assert!(blocked_exit_message.contains("post.loop.start"));
        assert!(blocked_exit_message.contains("hook exited with code 42"));

        let blocked_exec_outcomes = vec![HookDispatchOutcome {
            phase_event: HookPhaseEvent::PreLoopStart,
            hook_name: "block-exec-hook".to_string(),
            disposition: HookDisposition::Block,
            suspend_mode: HookSuspendMode::WaitForResume,
            failure: Some(HookDispatchFailure::HookExecutionError {
                message: "spawn failed".to_string(),
            }),

            mutation_parse_outcome: HookMutationParseOutcome::Disabled,
        }];

        let blocked_exec_error = fail_if_blocking_loop_start_outcomes(&blocked_exec_outcomes)
            .expect_err("block disposition should fail loop.start boundary");
        let blocked_exec_message = blocked_exec_error.to_string();
        assert!(blocked_exec_message.contains("block-exec-hook"));
        assert!(blocked_exec_message.contains("pre.loop.start"));
        assert!(blocked_exec_message.contains("hook execution failed: spawn failed"));
    }

    #[test]
    fn test_fail_if_blocking_iteration_start_outcomes_allows_non_blocking_dispositions() {
        let outcomes = vec![
            HookDispatchOutcome {
                phase_event: HookPhaseEvent::PreIterationStart,
                hook_name: "warn-hook".to_string(),
                disposition: HookDisposition::Warn,
                suspend_mode: HookSuspendMode::WaitForResume,
                failure: Some(HookDispatchFailure::HookRunFailed {
                    exit_code: Some(9),
                    timed_out: false,
                }),

                mutation_parse_outcome: HookMutationParseOutcome::Disabled,
            },
            HookDispatchOutcome {
                phase_event: HookPhaseEvent::PostIterationStart,
                hook_name: "pass-hook".to_string(),
                disposition: HookDisposition::Pass,
                suspend_mode: HookSuspendMode::WaitForResume,
                failure: None,

                mutation_parse_outcome: HookMutationParseOutcome::Disabled,
            },
        ];

        assert!(fail_if_blocking_iteration_start_outcomes(&outcomes).is_ok());
    }

    #[test]
    fn test_fail_if_blocking_iteration_start_outcomes_surfaces_failure_context() {
        let blocked_timeout_outcomes = vec![HookDispatchOutcome {
            phase_event: HookPhaseEvent::PreIterationStart,
            hook_name: "block-timeout-hook".to_string(),
            disposition: HookDisposition::Block,
            suspend_mode: HookSuspendMode::WaitForResume,
            failure: Some(HookDispatchFailure::HookRunFailed {
                exit_code: None,
                timed_out: true,
            }),

            mutation_parse_outcome: HookMutationParseOutcome::Disabled,
        }];

        let blocked_timeout_error =
            fail_if_blocking_iteration_start_outcomes(&blocked_timeout_outcomes)
                .expect_err("block disposition should fail iteration.start boundary");
        let blocked_timeout_message = blocked_timeout_error.to_string();
        assert!(blocked_timeout_message.contains("block-timeout-hook"));
        assert!(blocked_timeout_message.contains("pre.iteration.start"));
        assert!(blocked_timeout_message.contains("hook timed out"));

        let blocked_exec_outcomes = vec![HookDispatchOutcome {
            phase_event: HookPhaseEvent::PostIterationStart,
            hook_name: "block-exec-hook".to_string(),
            disposition: HookDisposition::Block,
            suspend_mode: HookSuspendMode::WaitForResume,
            failure: Some(HookDispatchFailure::HookExecutionError {
                message: "spawn failed".to_string(),
            }),

            mutation_parse_outcome: HookMutationParseOutcome::Disabled,
        }];

        let blocked_exec_error = fail_if_blocking_iteration_start_outcomes(&blocked_exec_outcomes)
            .expect_err("block disposition should fail iteration.start boundary");
        let blocked_exec_message = blocked_exec_error.to_string();
        assert!(blocked_exec_message.contains("block-exec-hook"));
        assert!(blocked_exec_message.contains("post.iteration.start"));
        assert!(blocked_exec_message.contains("hook execution failed: spawn failed"));
    }

    #[test]
    fn test_fail_if_blocking_human_interact_outcomes_allows_non_blocking_dispositions() {
        let outcomes = vec![
            HookDispatchOutcome {
                phase_event: HookPhaseEvent::PreHumanInteract,
                hook_name: "warn-hook".to_string(),
                disposition: HookDisposition::Warn,
                suspend_mode: HookSuspendMode::WaitForResume,
                failure: Some(HookDispatchFailure::HookRunFailed {
                    exit_code: Some(9),
                    timed_out: false,
                }),

                mutation_parse_outcome: HookMutationParseOutcome::Disabled,
            },
            HookDispatchOutcome {
                phase_event: HookPhaseEvent::PostHumanInteract,
                hook_name: "pass-hook".to_string(),
                disposition: HookDisposition::Pass,
                suspend_mode: HookSuspendMode::WaitForResume,
                failure: None,

                mutation_parse_outcome: HookMutationParseOutcome::Disabled,
            },
        ];

        assert!(fail_if_blocking_human_interact_outcomes(&outcomes).is_ok());
    }

    #[test]
    fn test_fail_if_blocking_human_interact_outcomes_surfaces_failure_context() {
        let blocked_timeout_outcomes = vec![HookDispatchOutcome {
            phase_event: HookPhaseEvent::PostHumanInteract,
            hook_name: "block-timeout-hook".to_string(),
            disposition: HookDisposition::Block,
            suspend_mode: HookSuspendMode::WaitForResume,
            failure: Some(HookDispatchFailure::HookRunFailed {
                exit_code: None,
                timed_out: true,
            }),

            mutation_parse_outcome: HookMutationParseOutcome::Disabled,
        }];

        let blocked_timeout_error =
            fail_if_blocking_human_interact_outcomes(&blocked_timeout_outcomes)
                .expect_err("block disposition should fail human.interact boundary");
        let blocked_timeout_message = blocked_timeout_error.to_string();
        assert!(blocked_timeout_message.contains("block-timeout-hook"));
        assert!(blocked_timeout_message.contains("post.human.interact"));
        assert!(blocked_timeout_message.contains("hook timed out"));

        let blocked_exec_outcomes = vec![HookDispatchOutcome {
            phase_event: HookPhaseEvent::PreHumanInteract,
            hook_name: "block-exec-hook".to_string(),
            disposition: HookDisposition::Block,
            suspend_mode: HookSuspendMode::WaitForResume,
            failure: Some(HookDispatchFailure::HookExecutionError {
                message: "spawn failed".to_string(),
            }),

            mutation_parse_outcome: HookMutationParseOutcome::Disabled,
        }];

        let blocked_exec_error = fail_if_blocking_human_interact_outcomes(&blocked_exec_outcomes)
            .expect_err("block disposition should fail human.interact boundary");
        let blocked_exec_message = blocked_exec_error.to_string();
        assert!(blocked_exec_message.contains("block-exec-hook"));
        assert!(blocked_exec_message.contains("pre.human.interact"));
        assert!(blocked_exec_message.contains("hook execution failed: spawn failed"));
    }

    #[test]
    fn test_loop_termination_phase_events_maps_success_and_error_reasons() {
        assert_eq!(
            loop_termination_phase_events(&TerminationReason::CompletionPromise),
            (
                HookPhaseEvent::PreLoopComplete,
                HookPhaseEvent::PostLoopComplete
            )
        );
        assert_eq!(
            loop_termination_phase_events(&TerminationReason::MaxRuntime),
            (HookPhaseEvent::PreLoopError, HookPhaseEvent::PostLoopError)
        );
    }

    #[test]
    fn test_build_loop_termination_payload_input_sets_termination_reason_context() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let loop_ctx = LoopContext::primary(temp_dir.path().to_path_buf());

        let payload_input = build_loop_termination_payload_input(
            "loop-test",
            &loop_ctx,
            42,
            7,
            Some("planner".to_string()),
            Some("builder".to_string()),
            Some("task-123".to_string()),
            &TerminationReason::RestartRequested,
        );

        assert_eq!(
            payload_input.context.termination_reason.as_deref(),
            Some("restart_requested")
        );
        assert_eq!(payload_input.context.active_hat.as_deref(), Some("planner"));
        assert_eq!(
            payload_input.context.selected_hat.as_deref(),
            Some("builder")
        );
        assert_eq!(
            payload_input.context.selected_task.as_deref(),
            Some("task-123")
        );
    }

    fn hook_mutation_config(enabled: bool) -> HookMutationConfig {
        HookMutationConfig {
            enabled,
            format: Some("json".to_string()),
            extra: std::collections::HashMap::new(),
        }
    }

    fn json_object(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().cloned().expect("json object")
    }

    #[test]
    fn test_parse_hook_mutation_stdout_skips_when_disabled() {
        let outcome =
            parse_hook_mutation_stdout(&HookMutationConfig::default(), "env-guard", "not-json");

        assert_eq!(outcome, HookMutationParseOutcome::Disabled);
    }

    #[test]
    fn test_parse_hook_mutation_stdout_accepts_metadata_only_payload_and_namespaces_by_hook() {
        let outcome = parse_hook_mutation_stdout(
            &hook_mutation_config(true),
            "env-guard",
            r#"{"metadata":{"risk_score":0.72,"gates":["policy_check"]}}"#,
        );

        let HookMutationParseOutcome::Parsed {
            namespaced_metadata,
        } = outcome
        else {
            panic!("expected parsed mutation payload");
        };

        assert_eq!(
            serde_json::Value::Object(namespaced_metadata),
            serde_json::json!({
                "hook_metadata": {
                    "env-guard": {
                        "risk_score": 0.72,
                        "gates": ["policy_check"]
                    }
                }
            })
        );
    }

    #[test]
    fn test_parse_hook_mutation_stdout_rejects_non_json_payload_when_enabled() {
        let outcome = parse_hook_mutation_stdout(&hook_mutation_config(true), "env-guard", "oops");

        let HookMutationParseOutcome::Invalid(HookMutationParseError::InvalidJson { message }) =
            outcome
        else {
            panic!("expected invalid-json mutation parse outcome");
        };

        assert!(message.contains("valid JSON"));
    }

    #[test]
    fn test_parse_hook_mutation_stdout_rejects_non_metadata_payload_shape() {
        let outcome = parse_hook_mutation_stdout(
            &hook_mutation_config(true),
            "env-guard",
            r#"{"metadata":{"risk_score":0.72},"prompt":"inject"}"#,
        );

        let HookMutationParseOutcome::Invalid(HookMutationParseError::InvalidSchema { message }) =
            outcome
        else {
            panic!("expected invalid-schema mutation parse outcome");
        };

        assert!(message.contains("supports only"));
    }

    #[test]
    fn test_merge_hook_metadata_namespace_merges_multiple_hook_entries() {
        let mut accumulated_metadata = serde_json::Map::new();
        accumulated_metadata.insert("upstream".to_string(), serde_json::json!("preserved"));

        merge_hook_metadata_namespace(
            &mut accumulated_metadata,
            "env-guard",
            json_object(serde_json::json!({"risk_score": 0.72})),
        )
        .expect("merge env-guard metadata");

        merge_hook_metadata_namespace(
            &mut accumulated_metadata,
            "policy-gate",
            json_object(serde_json::json!({"status": "pass"})),
        )
        .expect("merge policy-gate metadata");

        assert_eq!(
            accumulated_metadata["upstream"],
            serde_json::json!("preserved")
        );
        assert_eq!(
            accumulated_metadata["hook_metadata"]["env-guard"]["risk_score"],
            serde_json::json!(0.72)
        );
        assert_eq!(
            accumulated_metadata["hook_metadata"]["policy-gate"]["status"],
            serde_json::json!("pass")
        );
    }

    #[test]
    fn test_merge_hook_metadata_namespace_rejects_non_object_namespace_value() {
        let mut accumulated_metadata = serde_json::Map::new();
        accumulated_metadata.insert(
            "hook_metadata".to_string(),
            serde_json::Value::String("invalid".to_string()),
        );

        let merge_result = merge_hook_metadata_namespace(
            &mut accumulated_metadata,
            "env-guard",
            json_object(serde_json::json!({"risk_score": 0.72})),
        );

        assert!(matches!(
            merge_result,
            Err(HookMutationParseError::InvalidSchema { message })
            if message.contains("must be a JSON object")
        ));
    }

    #[test]
    fn test_fail_if_blocking_loop_termination_outcomes_allows_non_blocking_dispositions() {
        let outcomes = vec![
            HookDispatchOutcome {
                phase_event: HookPhaseEvent::PreLoopComplete,
                hook_name: "warn-hook".to_string(),
                disposition: HookDisposition::Warn,
                suspend_mode: HookSuspendMode::WaitForResume,
                failure: Some(HookDispatchFailure::HookRunFailed {
                    exit_code: Some(9),
                    timed_out: false,
                }),

                mutation_parse_outcome: HookMutationParseOutcome::Disabled,
            },
            HookDispatchOutcome {
                phase_event: HookPhaseEvent::PostLoopError,
                hook_name: "pass-hook".to_string(),
                disposition: HookDisposition::Pass,
                suspend_mode: HookSuspendMode::WaitForResume,
                failure: None,

                mutation_parse_outcome: HookMutationParseOutcome::Disabled,
            },
        ];

        assert!(fail_if_blocking_loop_termination_outcomes(&outcomes).is_ok());
    }

    #[test]
    fn test_fail_if_blocking_loop_termination_outcomes_surfaces_failure_context() {
        let blocked_timeout_outcomes = vec![HookDispatchOutcome {
            phase_event: HookPhaseEvent::PostLoopError,
            hook_name: "block-timeout-hook".to_string(),
            disposition: HookDisposition::Block,
            suspend_mode: HookSuspendMode::WaitForResume,
            failure: Some(HookDispatchFailure::HookRunFailed {
                exit_code: None,
                timed_out: true,
            }),

            mutation_parse_outcome: HookMutationParseOutcome::Disabled,
        }];

        let blocked_timeout_error =
            fail_if_blocking_loop_termination_outcomes(&blocked_timeout_outcomes)
                .expect_err("block disposition should fail loop termination boundary");
        let blocked_timeout_message = blocked_timeout_error.to_string();
        assert!(blocked_timeout_message.contains("block-timeout-hook"));
        assert!(blocked_timeout_message.contains("post.loop.error"));
        assert!(blocked_timeout_message.contains("hook timed out"));

        let blocked_exec_outcomes = vec![HookDispatchOutcome {
            phase_event: HookPhaseEvent::PreLoopComplete,
            hook_name: "block-exec-hook".to_string(),
            disposition: HookDisposition::Block,
            suspend_mode: HookSuspendMode::WaitForResume,
            failure: Some(HookDispatchFailure::HookExecutionError {
                message: "spawn failed".to_string(),
            }),

            mutation_parse_outcome: HookMutationParseOutcome::Disabled,
        }];

        let blocked_exec_error = fail_if_blocking_loop_termination_outcomes(&blocked_exec_outcomes)
            .expect_err("block disposition should fail loop termination boundary");
        let blocked_exec_message = blocked_exec_error.to_string();
        assert!(blocked_exec_message.contains("block-exec-hook"));
        assert!(blocked_exec_message.contains("pre.loop.complete"));
        assert!(blocked_exec_message.contains("hook execution failed: spawn failed"));
    }

    #[test]
    fn test_wait_for_resume_if_suspended_is_noop_without_suspend_dispositions() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let suspend_state_store = SuspendStateStore::new(temp_dir.path());

        let outcomes = vec![HookDispatchOutcome {
            phase_event: HookPhaseEvent::PreLoopStart,
            hook_name: "warn-hook".to_string(),
            disposition: HookDisposition::Warn,
            suspend_mode: HookSuspendMode::WaitForResume,
            failure: Some(HookDispatchFailure::HookRunFailed {
                exit_code: Some(7),
                timed_out: false,
            }),

            mutation_parse_outcome: HookMutationParseOutcome::Disabled,
        }];

        let wait_result = block_on_test_future(wait_for_resume_if_suspended(
            &outcomes,
            "loop-test",
            &suspend_state_store,
        ))
        .expect("wait helper should succeed");

        assert_eq!(wait_result, None);
        assert!(!suspend_state_store.suspend_state_path().exists());
        assert!(!suspend_state_store.resume_requested_path().exists());
    }

    #[test]
    fn test_wait_for_resume_if_suspended_resumes_and_clears_suspend_artifacts() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let suspend_state_store = SuspendStateStore::new(temp_dir.path());
        suspend_state_store
            .write_resume_requested()
            .expect("write resume signal");

        let outcomes = vec![suspend_outcome(
            HookPhaseEvent::PreLoopStart,
            "suspend-hook",
        )];

        let wait_result = block_on_test_future(wait_for_resume_if_suspended(
            &outcomes,
            "loop-test",
            &suspend_state_store,
        ))
        .expect("wait helper should succeed");

        assert_eq!(wait_result, None);
        assert!(!suspend_state_store.suspend_state_path().exists());
        assert!(!suspend_state_store.resume_requested_path().exists());
    }

    #[test]
    fn test_wait_for_resume_if_suspended_prioritizes_stop_over_resume() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let suspend_state_store = SuspendStateStore::new(temp_dir.path());

        std::fs::create_dir_all(temp_dir.path().join(".ralph")).expect("create .ralph");
        std::fs::write(temp_dir.path().join(".ralph/stop-requested"), "")
            .expect("write stop signal");
        suspend_state_store
            .write_resume_requested()
            .expect("write resume signal");

        let outcomes = vec![suspend_outcome(
            HookPhaseEvent::PreIterationStart,
            "suspend-hook",
        )];

        let wait_result = block_on_test_future(wait_for_resume_if_suspended(
            &outcomes,
            "loop-test",
            &suspend_state_store,
        ))
        .expect("wait helper should succeed");

        assert_eq!(wait_result, Some(TerminationReason::Stopped));
        assert!(!temp_dir.path().join(".ralph/stop-requested").exists());
        assert!(!suspend_state_store.suspend_state_path().exists());
        assert!(!suspend_state_store.resume_requested_path().exists());
    }

    #[test]
    fn test_wait_for_resume_if_suspended_prioritizes_restart_over_resume() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let suspend_state_store = SuspendStateStore::new(temp_dir.path());

        std::fs::create_dir_all(temp_dir.path().join(".ralph")).expect("create .ralph");
        std::fs::write(temp_dir.path().join(".ralph/restart-requested"), "")
            .expect("write restart signal");
        suspend_state_store
            .write_resume_requested()
            .expect("write resume signal");

        let outcomes = vec![suspend_outcome(
            HookPhaseEvent::PostIterationStart,
            "suspend-hook",
        )];

        let wait_result = block_on_test_future(wait_for_resume_if_suspended(
            &outcomes,
            "loop-test",
            &suspend_state_store,
        ))
        .expect("wait helper should succeed");

        assert_eq!(wait_result, Some(TerminationReason::RestartRequested));
        assert!(temp_dir.path().join(".ralph/restart-requested").exists());
        assert!(!suspend_state_store.suspend_state_path().exists());
        assert!(!suspend_state_store.resume_requested_path().exists());
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

    #[test]
    fn test_detect_solo_output_completion_requires_hatless_mode() {
        let registry = HatRegistry::new();
        assert!(detect_solo_output_completion(
            &registry,
            "done\nLOOP_COMPLETE\n",
            "LOOP_COMPLETE"
        ));

        let yaml = r#"
hats:
  builder:
    name: "Builder"
    triggers: ["build.start"]
    publishes: ["build.done"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        assert!(
            !detect_solo_output_completion(&registry, "done\nLOOP_COMPLETE\n", "LOOP_COMPLETE"),
            "text completion should not terminate multi-hat workflows"
        );
    }

    #[test]
    fn test_detect_solo_output_completion_requires_final_non_empty_line() {
        let registry = HatRegistry::new();
        assert!(!detect_solo_output_completion(
            &registry,
            "LOOP_COMPLETE\nMore text after\n",
            "LOOP_COMPLETE"
        ));
        assert!(!detect_solo_output_completion(
            &registry,
            "I think LOOP_COMPLETE but not really",
            "LOOP_COMPLETE"
        ));
    }

    #[test]
    fn test_normalize_cli_output_for_parsing_extracts_claude_text_blocks() {
        let raw = concat!(
            "{\"type\":\"system\",\"session_id\":\"abc\",\"model\":\"claude-opus-4-6\",\"tools\":[]}\n",
            "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"First line\"}]}}\n",
            "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"id\":\"tool_1\",\"name\":\"Bash\",\"input\":{\"command\":\"pytest\"}}]}}\n",
            "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"tool_1\",\"content\":\"ok\"}]}}\n",
            "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"LOOP_COMPLETE\"}]}}\n",
            "{\"type\":\"result\",\"duration_ms\":1,\"total_cost_usd\":0.0,\"num_turns\":1,\"is_error\":false}\n"
        );

        assert_eq!(
            normalize_cli_output_for_parsing(BackendOutputFormat::StreamJson, raw),
            "First line\nLOOP_COMPLETE\n"
        );
    }

    #[test]
    fn test_normalize_cli_output_for_parsing_extracts_pi_text_deltas() {
        let raw = concat!(
            "{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"text_delta\",\"contentIndex\":0,\"delta\":\"hello \"}}\n",
            "{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"thinking_delta\",\"contentIndex\":0,\"delta\":\"hidden\"}}\n",
            "{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"text_delta\",\"contentIndex\":0,\"delta\":\"LOOP_COMPLETE\"}}\n",
            "{\"type\":\"turn_end\",\"message\":{\"usage\":{\"input\":1,\"output\":1,\"cache_read\":0,\"cache_write\":0,\"cost\":{\"input\":0.0,\"output\":0.0,\"total\":0.0}}}}\n"
        );

        assert_eq!(
            normalize_cli_output_for_parsing(BackendOutputFormat::PiStreamJson, raw),
            "hello LOOP_COMPLETE"
        );
    }

    #[test]
    fn test_wave_worker_execution_mode_supports_all_backend_formats() {
        assert_eq!(
            wave_worker_execution_mode(BackendOutputFormat::Text),
            WaveWorkerExecutionMode::Pty
        );
        assert_eq!(
            wave_worker_execution_mode(BackendOutputFormat::StreamJson),
            WaveWorkerExecutionMode::Pty
        );
        assert_eq!(
            wave_worker_execution_mode(BackendOutputFormat::PiStreamJson),
            WaveWorkerExecutionMode::Pty
        );
        assert_eq!(
            wave_worker_execution_mode(BackendOutputFormat::Acp),
            WaveWorkerExecutionMode::Acp
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_wave_worker_execution_mode_matches_supported_named_backend_roster() {
        for (name, expected_output_format, expected_mode, marker_id) in [
            (
                "claude",
                BackendOutputFormat::StreamJson,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:named:claude",
            ),
            (
                "pi",
                BackendOutputFormat::PiStreamJson,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:named:pi",
            ),
            (
                "kiro-acp",
                BackendOutputFormat::Acp,
                WaveWorkerExecutionMode::Acp,
                "execution-mode:named:kiro-acp",
            ),
            (
                "kiro",
                BackendOutputFormat::Text,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:named:kiro",
            ),
            (
                "gemini",
                BackendOutputFormat::Text,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:named:gemini",
            ),
            (
                "codex",
                BackendOutputFormat::Text,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:named:codex",
            ),
            (
                "amp",
                BackendOutputFormat::Text,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:named:amp",
            ),
            (
                "copilot",
                BackendOutputFormat::Text,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:named:copilot",
            ),
            (
                "opencode",
                BackendOutputFormat::Text,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:named:opencode",
            ),
            (
                "roo",
                BackendOutputFormat::Text,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:named:roo",
            ),
        ] {
            let backend = CliBackend::from_name(name).expect("supported named backend");
            assert_eq!(
                backend.output_format, expected_output_format,
                "unexpected output format for {name}"
            );
            assert_eq!(
                wave_worker_execution_mode(backend.output_format),
                expected_mode,
                "unexpected wave worker execution mode for {name}"
            );
            emit_wave_validation_marker(marker_id, &["backend"]);
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_wave_worker_execution_mode_matches_supported_hat_backend_families() {
        for (hat_backend, expected_output_format, expected_mode, marker_id) in [
            (
                ralph_core::HatBackend::Named("claude".to_string()),
                BackendOutputFormat::StreamJson,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:hat:named-claude",
            ),
            (
                ralph_core::HatBackend::NamedWithArgs {
                    backend_type: "opencode".to_string(),
                    args: vec!["--from-hat-backend".to_string()],
                },
                BackendOutputFormat::Text,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:hat:named-with-args",
            ),
            (
                ralph_core::HatBackend::KiroAgent {
                    backend_type: "kiro".to_string(),
                    agent: "reviewer-agent".to_string(),
                    args: vec!["--kiro-extra".to_string()],
                },
                BackendOutputFormat::Text,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:hat:kiro-agent",
            ),
            (
                ralph_core::HatBackend::KiroAgent {
                    backend_type: "kiro-acp".to_string(),
                    agent: "reviewer-agent".to_string(),
                    args: vec!["--unused-extra".to_string()],
                },
                BackendOutputFormat::Acp,
                WaveWorkerExecutionMode::Acp,
                "execution-mode:hat:kiro-acp-agent",
            ),
            (
                ralph_core::HatBackend::Custom {
                    command: "/tmp/custom-wave-worker".to_string(),
                    args: vec!["--from-custom-backend".to_string()],
                },
                BackendOutputFormat::Text,
                WaveWorkerExecutionMode::Pty,
                "execution-mode:hat:custom",
            ),
        ] {
            let backend =
                CliBackend::from_hat_backend(&hat_backend).expect("supported hat backend");
            assert_eq!(
                backend.output_format, expected_output_format,
                "unexpected output format for {hat_backend:?}"
            );
            assert_eq!(
                wave_worker_execution_mode(backend.output_format),
                expected_mode,
                "unexpected wave worker execution mode for {hat_backend:?}"
            );
            emit_wave_validation_marker(marker_id, &["backend"]);
        }
    }

    #[test]
    fn test_extract_readable_delta_handles_pi_stream_events() {
        let text_delta = extract_readable_delta(
            "{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"text_delta\",\"contentIndex\":0,\"delta\":\"Hello from Pi\"}}",
            BackendOutputFormat::PiStreamJson,
        );
        assert_eq!(text_delta.as_deref(), Some("Hello from Pi"));

        let tool_delta = extract_readable_delta(
            "{\"type\":\"tool_execution_start\",\"toolCallId\":\"toolu_1\",\"toolName\":\"bash\",\"args\":{\"command\":\"echo hi\"}}",
            BackendOutputFormat::PiStreamJson,
        )
        .expect("pi tool start delta");
        assert!(tool_delta.contains("⚙ bash"));
        assert!(tool_delta.contains("echo hi"));

        let result_delta = extract_readable_delta(
            "{\"type\":\"tool_execution_end\",\"toolCallId\":\"toolu_1\",\"toolName\":\"bash\",\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"hi\\n\"}]},\"isError\":false}",
            BackendOutputFormat::PiStreamJson,
        );
        assert_eq!(result_delta.as_deref(), Some("→ hi\n\n"));
    }

    #[cfg(unix)]
    fn make_test_wave(publishes: Vec<String>) -> ralph_core::DetectedWave {
        make_test_wave_with_timeout(publishes, 30)
    }

    #[cfg(unix)]
    fn make_test_wave_with_timeout(
        publishes: Vec<String>,
        timeout_secs: u32,
    ) -> ralph_core::DetectedWave {
        make_test_wave_with_timeout_and_payload(
            publishes,
            timeout_secs,
            "ROLE: Validate this backend".to_string(),
        )
    }

    #[cfg(unix)]
    fn make_test_wave_with_timeout_and_payload(
        publishes: Vec<String>,
        timeout_secs: u32,
        payload: String,
    ) -> ralph_core::DetectedWave {
        let event = ralph_core::Event {
            topic: "review.perspective".to_string(),
            payload: Some(payload),
            ts: "2026-01-01T00:00:00Z".to_string(),
            wave_id: Some("w-test".to_string()),
            wave_index: Some(0),
            wave_total: Some(1),
        };

        ralph_core::DetectedWave {
            wave_id: "w-test".to_string(),
            target_hat: "reviewer".into(),
            hat_config: ralph_core::HatConfig {
                name: "Reviewer".to_string(),
                description: Some("Wave worker test".to_string()),
                triggers: vec!["review.perspective".to_string()],
                publishes,
                instructions: "Emit review.done when finished.".to_string(),
                extra_instructions: vec![],
                backend: None,
                backend_args: None,
                default_publishes: None,
                max_activations: None,
                disallowed_tools: vec![],
                timeout: Some(timeout_secs),
                concurrency: 1,
                aggregate: None,
            },
            events: vec![event],
            total: 1,
        }
    }

    #[cfg(unix)]
    async fn run_wave_for_backend(
        output_format: BackendOutputFormat,
        body: &str,
    ) -> ralph_core::CompletedWave {
        run_wave_for_backend_with_timeout(output_format, body, 30).await
    }

    #[cfg(unix)]
    async fn run_wave_for_backend_with_timeout(
        output_format: BackendOutputFormat,
        body: &str,
        timeout_secs: u32,
    ) -> ralph_core::CompletedWave {
        run_wave_for_backend_with_test_env(output_format, body, timeout_secs, vec![]).await
    }

    #[cfg(unix)]
    async fn run_wave_for_backend_with_test_env(
        output_format: BackendOutputFormat,
        body: &str,
        timeout_secs: u32,
        env_vars: Vec<(&str, &str)>,
    ) -> ralph_core::CompletedWave {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let worker_path = write_fake_executable(&bin_dir, "wave-worker", body);

        let backend = CliBackend {
            command: worker_path.display().to_string(),
            args: vec![],
            prompt_mode: ralph_adapters::PromptMode::Arg,
            prompt_flag: None,
            output_format,
            env_vars: env_vars
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        };

        let events_file = temp_dir.path().join("events.jsonl");
        let wave = make_test_wave_with_timeout(vec!["review.done".to_string()], timeout_secs);
        execute_wave(&wave, &backend, &events_file, false, false, None, None)
            .await
            .expect("wave execution")
    }

    #[cfg(unix)]
    async fn run_wave_for_named_backend(name: &str, body: &str) -> ralph_core::CompletedWave {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");

        let mut backend = CliBackend::from_name(name).expect("named backend");
        let executable_name = Path::new(&backend.command)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(backend.command.as_str())
            .to_string();
        write_fake_executable(&bin_dir, &executable_name, body);

        let existing_path = std::env::var("PATH").unwrap_or_default();
        let path_value = if existing_path.is_empty() {
            bin_dir.display().to_string()
        } else {
            format!("{}:{}", bin_dir.display(), existing_path)
        };
        backend.env_vars.push(("PATH".to_string(), path_value));

        let events_file = temp_dir.path().join("events.jsonl");
        let wave = make_test_wave(vec!["review.done".to_string()]);
        execute_wave(&wave, &backend, &events_file, false, false, None, None)
            .await
            .expect("wave execution")
    }

    #[cfg(unix)]
    #[derive(Debug, serde::Deserialize)]
    struct CapturedWaveInvocation {
        args: Vec<String>,
        env: std::collections::BTreeMap<String, String>,
        prompt: String,
    }

    #[cfg(unix)]
    async fn run_wave_for_named_backend_with_capture(
        name: &str,
        payload: &str,
    ) -> (ralph_core::CompletedWave, CapturedWaveInvocation) {
        run_wave_for_named_backend_with_capture_and_task_payload(
            name,
            payload,
            "ROLE: Validate this backend",
        )
        .await
    }

    #[cfg(unix)]
    async fn run_wave_for_named_backend_with_capture_and_task_payload(
        name: &str,
        payload: &str,
        task_payload: &str,
    ) -> (ralph_core::CompletedWave, CapturedWaveInvocation) {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");

        let worker_capture_path = temp_dir.path().join("wave-w-test-0.jsonl.capture");
        let mut backend = CliBackend::from_name(name).expect("named backend");
        let executable_name = Path::new(&backend.command)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(backend.command.as_str())
            .to_string();
        write_fake_executable(
            &bin_dir,
            &executable_name,
            &invocation_capture_backend_body(payload),
        );

        let existing_path = std::env::var("PATH").unwrap_or_default();
        let path_value = if existing_path.is_empty() {
            bin_dir.display().to_string()
        } else {
            format!("{}:{}", bin_dir.display(), existing_path)
        };
        backend.env_vars.push(("PATH".to_string(), path_value));

        let events_file = temp_dir.path().join("events.jsonl");
        let wave = make_test_wave_with_timeout_and_payload(
            vec!["review.done".to_string()],
            30,
            task_payload.to_string(),
        );
        let completed = execute_wave(&wave, &backend, &events_file, false, false, None, None)
            .await
            .expect("wave execution");
        let captured: CapturedWaveInvocation = serde_json::from_str(
            &std::fs::read_to_string(&worker_capture_path).expect("read captured invocation"),
        )
        .expect("parse captured invocation");
        (completed, captured)
    }

    #[cfg(unix)]
    fn missing_global_wave_backend() -> CliBackend {
        CliBackend {
            command: "/definitely/missing-wave-worker".to_string(),
            args: vec![],
            prompt_mode: ralph_adapters::PromptMode::Arg,
            prompt_flag: None,
            output_format: BackendOutputFormat::Text,
            env_vars: vec![],
        }
    }

    #[cfg(unix)]
    async fn run_wave_for_hat_backend(
        hat_backend: ralph_core::HatBackend,
        backend_args: Option<Vec<String>>,
        global_backend: CliBackend,
    ) -> ralph_core::CompletedWave {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let events_file = temp_dir.path().join("events.jsonl");
        let mut wave = make_test_wave(vec!["review.done".to_string()]);
        wave.hat_config.backend = Some(hat_backend);
        wave.hat_config.backend_args = backend_args;

        execute_wave(
            &wave,
            &global_backend,
            &events_file,
            false,
            false,
            None,
            None,
        )
        .await
        .expect("wave execution")
    }

    #[cfg(unix)]
    async fn run_wave_for_hat_backend_with_capture(
        hat_backend: ralph_core::HatBackend,
        backend_args: Option<Vec<String>>,
    ) -> (ralph_core::CompletedWave, CapturedWaveInvocation) {
        run_wave_for_hat_backend_with_capture_and_task_payload(
            hat_backend,
            backend_args,
            "ROLE: Validate this backend",
        )
        .await
    }

    #[cfg(unix)]
    async fn run_wave_for_hat_backend_with_capture_and_task_payload(
        hat_backend: ralph_core::HatBackend,
        backend_args: Option<Vec<String>>,
        task_payload: &str,
    ) -> (ralph_core::CompletedWave, CapturedWaveInvocation) {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let worker_capture_path = temp_dir.path().join("wave-w-test-0.jsonl.capture");
        let events_file = temp_dir.path().join("events.jsonl");
        let mut wave = make_test_wave_with_timeout_and_payload(
            vec!["review.done".to_string()],
            30,
            task_payload.to_string(),
        );
        wave.hat_config.backend = Some(hat_backend);
        wave.hat_config.backend_args = backend_args;

        let completed = execute_wave(
            &wave,
            &missing_global_wave_backend(),
            &events_file,
            false,
            false,
            None,
            None,
        )
        .await
        .expect("wave execution");
        let captured: CapturedWaveInvocation = serde_json::from_str(
            &std::fs::read_to_string(&worker_capture_path).expect("read captured invocation"),
        )
        .expect("parse captured invocation");
        (completed, captured)
    }

    #[cfg(unix)]
    #[derive(Debug, serde::Deserialize)]
    struct CapturedAcpWaveInvocation {
        command: String,
        args: Vec<String>,
        env: std::collections::BTreeMap<String, String>,
        prompt: String,
    }

    #[cfg(unix)]
    async fn run_wave_for_named_acp_backend_with_capture(
        backend_args: Option<Vec<String>>,
        payload: &str,
    ) -> (ralph_core::CompletedWave, CapturedAcpWaveInvocation) {
        let _mock = install_mock_acp_executions(vec![MockAcpExecution::success(
            true,
            vec![make_worker_event("review.done", payload)],
        )]);
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let worker_capture_path = temp_dir.path().join("wave-w-test-0.jsonl.capture");
        let events_file = temp_dir.path().join("events.jsonl");
        let mut wave = make_test_wave(vec!["review.done".to_string()]);
        wave.hat_config.backend_args = backend_args;
        let backend = CliBackend::from_name("kiro-acp").expect("named ACP backend");

        let completed = execute_wave(&wave, &backend, &events_file, false, false, None, None)
            .await
            .expect("wave execution");
        let captured: CapturedAcpWaveInvocation = serde_json::from_str(
            &std::fs::read_to_string(&worker_capture_path).expect("read captured ACP invocation"),
        )
        .expect("parse captured ACP invocation");
        (completed, captured)
    }

    #[cfg(unix)]
    async fn run_wave_for_hat_backend_with_acp_capture(
        hat_backend: ralph_core::HatBackend,
        backend_args: Option<Vec<String>>,
        payload: &str,
    ) -> (ralph_core::CompletedWave, CapturedAcpWaveInvocation) {
        run_wave_for_hat_backend_with_acp_capture_and_task_payload(
            hat_backend,
            backend_args,
            payload,
            "ROLE: Validate this backend",
        )
        .await
    }

    #[cfg(unix)]
    async fn run_wave_for_hat_backend_with_acp_capture_and_task_payload(
        hat_backend: ralph_core::HatBackend,
        backend_args: Option<Vec<String>>,
        payload: &str,
        task_payload: &str,
    ) -> (ralph_core::CompletedWave, CapturedAcpWaveInvocation) {
        let _mock = install_mock_acp_executions(vec![MockAcpExecution::success(
            true,
            vec![make_worker_event("review.done", payload)],
        )]);
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let worker_capture_path = temp_dir.path().join("wave-w-test-0.jsonl.capture");
        let events_file = temp_dir.path().join("events.jsonl");
        let mut wave = make_test_wave_with_timeout_and_payload(
            vec!["review.done".to_string()],
            30,
            task_payload.to_string(),
        );
        wave.hat_config.backend = Some(hat_backend);
        wave.hat_config.backend_args = backend_args;

        let completed = execute_wave(
            &wave,
            &missing_global_wave_backend(),
            &events_file,
            false,
            false,
            None,
            None,
        )
        .await
        .expect("wave execution");
        let captured: CapturedAcpWaveInvocation = serde_json::from_str(
            &std::fs::read_to_string(&worker_capture_path).expect("read captured ACP invocation"),
        )
        .expect("parse captured ACP invocation");
        (completed, captured)
    }

    #[cfg(unix)]
    fn make_worker_event(topic: &str, payload: &str) -> ralph_core::Event {
        ralph_core::Event {
            topic: topic.to_string(),
            payload: Some(payload.to_string()),
            ts: "2026-01-01T00:00:00Z".to_string(),
            wave_id: None,
            wave_index: None,
            wave_total: None,
        }
    }

    #[cfg(unix)]
    fn text_backend_body(payload: &str) -> String {
        format!(
            r#"printf 'plain text from worker\n'
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{{"topic":"review.done","payload":"{payload}","ts":"2026-01-01T00:00:00Z"}}
EOF"#,
        )
    }

    #[cfg(unix)]
    fn claude_backend_body(payload: &str) -> String {
        format!(
            r#"printf '%s\n' \
'{{"type":"assistant","message":{{"content":[{{"type":"text","text":"hello from named claude"}}]}}}}' \
'{{"type":"result","duration_ms":1,"total_cost_usd":0.0,"num_turns":1,"is_error":false}}'
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{{"topic":"review.done","payload":"{payload}","ts":"2026-01-01T00:00:00Z"}}
EOF"#,
        )
    }

    #[cfg(unix)]
    fn pi_backend_body(payload: &str) -> String {
        format!(
            r#"printf '%s\n' \
'{{"type":"message_update","assistantMessageEvent":{{"type":"text_delta","contentIndex":0,"delta":"hello from named pi"}}}}' \
'{{"type":"tool_execution_start","toolCallId":"toolu_1","toolName":"bash","args":{{"command":"echo hi"}}}}' \
'{{"type":"tool_execution_end","toolCallId":"toolu_1","toolName":"bash","result":{{"content":[{{"type":"text","text":"hi\n"}}]}},"isError":false}}'
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{{"topic":"review.done","payload":"{payload}","ts":"2026-01-01T00:00:00Z"}}
EOF"#,
        )
    }

    #[cfg(unix)]
    fn invocation_capture_backend_body(payload: &str) -> String {
        format!(
            r#"python3 -c '
import json
import os
import pathlib
import select
import sys

args = sys.argv[1:]
prompt = ""
if "--prompt-file" in args:
    prompt_flag_index = args.index("--prompt-file")
    prompt = pathlib.Path(args[prompt_flag_index + 1]).read_text()
elif "--print" in args:
    chunks = []
    fd = sys.stdin.fileno()
    while True:
        ready, _, _ = select.select([fd], [], [], 0.1)
        if not ready:
            break
        chunk = os.read(fd, 4096)
        if not chunk:
            break
        chunks.append(chunk)
    prompt = b"".join(chunks).decode()
elif args:
    prompt = args[-1]
    temp_file_prefix = "Please read and execute the task in "
    if prompt.startswith(temp_file_prefix):
        prompt = pathlib.Path(prompt[len(temp_file_prefix):]).read_text()

pathlib.Path(os.environ["RALPH_EVENTS_FILE"] + ".capture").write_text(json.dumps({{
    "args": args,
    "env": {{
        "RALPH_WAVE_WORKER": os.environ.get("RALPH_WAVE_WORKER", ""),
        "RALPH_WAVE_ID": os.environ.get("RALPH_WAVE_ID", ""),
        "RALPH_WAVE_INDEX": os.environ.get("RALPH_WAVE_INDEX", ""),
        "RALPH_EVENTS_FILE": os.environ.get("RALPH_EVENTS_FILE", ""),
        "TERM": os.environ.get("TERM", ""),
        "NO_COLOR": os.environ.get("NO_COLOR", ""),
    }},
    "prompt": prompt,
}}))
' "$@"
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{{"topic":"review.done","payload":"{payload}","ts":"2026-01-01T00:00:00Z"}}
EOF"#,
        )
    }

    #[cfg(unix)]
    enum PromptDeliveryExpectation {
        Flag(&'static str),
        Positional,
        Stdin,
        TempFileFlag(&'static str),
        TempFilePositional,
        PromptFile,
    }

    #[cfg(unix)]
    fn assert_captured_wave_prompt(prompt: &str) {
        assert!(
            prompt.contains("# Instructions"),
            "missing instructions: {prompt}"
        );
        assert!(
            prompt.contains("Emit review.done when finished."),
            "missing worker instructions: {prompt}"
        );
        assert!(
            prompt.contains("# Wave Context"),
            "missing wave context: {prompt}"
        );
        assert!(
            prompt.contains("worker **1/1**"),
            "missing worker index: {prompt}"
        );
        assert!(prompt.contains("w-test"), "missing wave id: {prompt}");
        assert!(
            prompt.contains("# Your Task"),
            "missing task section: {prompt}"
        );
        assert!(
            prompt.contains("ROLE: Validate this backend"),
            "missing task payload: {prompt}"
        );
        assert!(
            prompt.contains("ralph emit review.done"),
            "missing publishing guidance: {prompt}"
        );
        assert!(prompt.contains("DO NOT"), "missing constraints: {prompt}");
    }

    #[cfg(unix)]
    fn assert_captured_wave_env(
        env: &std::collections::BTreeMap<String, String>,
        expect_terminal_env: bool,
    ) {
        assert_eq!(env.get("RALPH_WAVE_WORKER").map(String::as_str), Some("1"));
        assert_eq!(env.get("RALPH_WAVE_ID").map(String::as_str), Some("w-test"));
        assert_eq!(env.get("RALPH_WAVE_INDEX").map(String::as_str), Some("0"));
        if expect_terminal_env {
            assert_eq!(env.get("TERM").map(String::as_str), Some("dumb"));
            assert_eq!(env.get("NO_COLOR").map(String::as_str), Some("1"));
        }
        assert!(
            env.get("RALPH_EVENTS_FILE")
                .is_some_and(|path| path.ends_with("wave-w-test-0.jsonl")),
            "missing wave events file env: {:?}",
            env
        );
    }

    #[cfg(unix)]
    fn assert_temp_file_prompt_instruction(instruction: &str, captured_prompt: &str) {
        let prefix = "Please read and execute the task in ";
        assert!(
            instruction.starts_with(prefix),
            "expected temp-file handoff instruction, got {instruction:?}"
        );
        let path = &instruction[prefix.len()..];
        assert!(
            !path.is_empty(),
            "missing temp-file path in {instruction:?}"
        );
        assert!(
            path.starts_with('/'),
            "expected absolute temp-file path, got {instruction:?}"
        );
        assert_ne!(
            captured_prompt, instruction,
            "captured prompt should contain temp-file contents, not the handoff instruction"
        );
    }

    #[cfg(unix)]
    fn assert_named_backend_invocation_contract(
        captured: &CapturedWaveInvocation,
        expected_prefix: &[&str],
        prompt_delivery: PromptDeliveryExpectation,
    ) {
        let args = captured.args.iter().map(String::as_str).collect::<Vec<_>>();

        assert_eq!(
            &args[..expected_prefix.len()],
            expected_prefix,
            "unexpected fixed args: {:?}",
            captured.args
        );

        match prompt_delivery {
            PromptDeliveryExpectation::Flag(flag) => {
                assert_eq!(
                    args.len(),
                    expected_prefix.len() + 2,
                    "unexpected arg count: {:?}",
                    captured.args
                );
                assert_eq!(args[expected_prefix.len()], flag, "missing prompt flag");
                assert_eq!(
                    captured.prompt,
                    args[expected_prefix.len() + 1],
                    "prompt arg should match captured prompt"
                );
            }
            PromptDeliveryExpectation::Positional => {
                assert_eq!(
                    args.len(),
                    expected_prefix.len() + 1,
                    "unexpected arg count: {:?}",
                    captured.args
                );
                assert_eq!(
                    captured.prompt,
                    args[expected_prefix.len()],
                    "positional prompt should match captured prompt"
                );
            }
            PromptDeliveryExpectation::Stdin => {
                assert_eq!(
                    args.len(),
                    expected_prefix.len(),
                    "unexpected arg count: {:?}",
                    captured.args
                );
                assert!(
                    !captured.prompt.is_empty(),
                    "stdin-delivered prompt should be captured"
                );
            }
            PromptDeliveryExpectation::TempFileFlag(flag) => {
                assert_eq!(
                    args.len(),
                    expected_prefix.len() + 2,
                    "unexpected arg count: {:?}",
                    captured.args
                );
                assert_eq!(args[expected_prefix.len()], flag, "missing prompt flag");
                assert_temp_file_prompt_instruction(
                    args[expected_prefix.len() + 1],
                    &captured.prompt,
                );
            }
            PromptDeliveryExpectation::TempFilePositional => {
                assert_eq!(
                    args.len(),
                    expected_prefix.len() + 1,
                    "unexpected arg count: {:?}",
                    captured.args
                );
                assert_temp_file_prompt_instruction(args[expected_prefix.len()], &captured.prompt);
            }
            PromptDeliveryExpectation::PromptFile => {
                assert_eq!(
                    args.len(),
                    expected_prefix.len() + 2,
                    "unexpected arg count: {:?}",
                    captured.args
                );
                assert_eq!(
                    args[expected_prefix.len()],
                    "--prompt-file",
                    "missing roo prompt file flag"
                );
                assert!(
                    !args[expected_prefix.len() + 1].is_empty(),
                    "missing roo prompt file path"
                );
                assert!(
                    args[expected_prefix.len() + 1].contains("tmp")
                        || args[expected_prefix.len() + 1].contains("Temp"),
                    "expected temp prompt file path, got {:?}",
                    captured.args
                );
            }
        }

        assert_captured_wave_prompt(&captured.prompt);
        assert_captured_wave_env(&captured.env, true);
    }

    #[cfg(unix)]
    fn assert_acp_invocation_contract(
        captured: &CapturedAcpWaveInvocation,
        expected_args: &[&str],
    ) {
        assert_eq!(captured.command, "kiro-cli");
        assert_eq!(
            captured.args.iter().map(String::as_str).collect::<Vec<_>>(),
            expected_args,
            "unexpected ACP args: {:?}",
            captured.args
        );
        assert_captured_wave_prompt(&captured.prompt);
        assert_captured_wave_env(&captured.env, false);
    }

    #[cfg(unix)]
    fn body_with_post_event_sleep(body: String) -> String {
        format!("{body}\npython3 - <<'PY'\nimport time\ntime.sleep(2)\nPY")
    }

    #[cfg(unix)]
    macro_rules! named_text_wave_backend_test {
        ($test_name:ident, $backend_name:literal, $payload:literal) => {
            #[tokio::test]
            async fn $test_name() {
                let completed =
                    run_wave_for_named_backend($backend_name, &text_backend_body($payload)).await;
                assert_single_success_marked(
                    &completed,
                    $payload,
                    concat!("named-backend:", $backend_name),
                );
            }
        };
    }

    #[cfg(unix)]
    fn assert_single_success(completed: &ralph_core::CompletedWave, expected_payload: &str) {
        assert!(
            completed.failures.is_empty(),
            "unexpected failures: {:?}",
            completed.failures
        );
        assert_eq!(
            completed.results.len(),
            1,
            "unexpected results: {:?}",
            completed.results
        );
        assert_eq!(completed.results[0].events.len(), 1);
        assert_eq!(completed.results[0].events[0].topic.as_str(), "review.done");
        assert_eq!(completed.results[0].events[0].payload, expected_payload);
    }

    #[cfg(unix)]
    fn emit_wave_validation_marker(id: &str, tags: &[&str]) {
        println!("WAVE_VALIDATION_MARKER id={id} tags={}", tags.join(","));
    }

    #[cfg(unix)]
    fn assert_single_success_marked(
        completed: &ralph_core::CompletedWave,
        expected_payload: &str,
        marker_id: &str,
    ) {
        assert_single_success(completed, expected_payload);
        emit_wave_validation_marker(marker_id, &["backend"]);
    }

    #[cfg(unix)]
    fn assert_single_failure_with_synthetic_events_marked(
        completed: &ralph_core::CompletedWave,
        expected_error: &str,
        marker_id: &str,
    ) {
        assert!(
            completed.results.is_empty(),
            "unexpected results: {completed:?}"
        );
        assert_eq!(
            completed.failures.len(),
            1,
            "unexpected failures: {completed:?}"
        );
        assert!(
            completed.failures[0].error.contains(expected_error),
            "unexpected failure: {:?}",
            completed.failures[0]
        );

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let merged_events_path = temp_dir.path().join("events.jsonl");
        merge_wave_results_to_events_file(
            completed,
            &merged_events_path,
            &["review.done".to_string(), "review.audit".to_string()],
        )
        .expect("merge wave failure results");

        let merged = std::fs::read_to_string(&merged_events_path).expect("read merged events");
        let records: Vec<serde_json::Value> = merged
            .lines()
            .map(|line| serde_json::from_str(line).expect("json event"))
            .collect();

        assert_eq!(records.len(), 3, "unexpected merged records: {records:?}");
        assert!(records.iter().any(|record| {
            record["topic"] == "wave.worker.failed"
                && record["payload"]
                    .as_str()
                    .is_some_and(|payload| payload.contains(expected_error))
                && record["wave_index"] == 0
        }));
        for topic in ["review.done", "review.audit"] {
            assert!(records.iter().any(|record| {
                record["topic"] == topic
                    && record["payload"].as_str().is_some_and(|payload| {
                        payload.contains("## Worker 0 (FAILED)") && payload.contains(expected_error)
                    })
            }));
        }

        emit_wave_validation_marker(marker_id, &["backend", "error", "synthetic"]);
    }

    #[cfg(unix)]
    fn assert_partial_timeout_events_visible_marked(
        completed: &ralph_core::CompletedWave,
        expected_payload: &str,
        marker_id: &str,
    ) {
        assert!(
            completed.failures.is_empty(),
            "unexpected failures: {completed:?}"
        );
        assert_eq!(
            completed.results.len(),
            1,
            "unexpected results: {completed:?}"
        );
        assert_eq!(
            completed.results[0].events.len(),
            1,
            "unexpected result events: {completed:?}"
        );
        assert_eq!(completed.results[0].events[0].topic.as_str(), "review.done");
        assert_eq!(completed.results[0].events[0].payload, expected_payload);

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let merged_events_path = temp_dir.path().join("events.jsonl");
        merge_wave_results_to_events_file(
            completed,
            &merged_events_path,
            &["review.done".to_string()],
        )
        .expect("merge partial-timeout results");

        let merged = std::fs::read_to_string(&merged_events_path).expect("read merged events");
        let records: Vec<serde_json::Value> = merged
            .lines()
            .map(|line| serde_json::from_str(line).expect("json event"))
            .collect();

        assert_eq!(records.len(), 1, "unexpected merged records: {records:?}");
        assert_eq!(records[0]["topic"], "review.done");
        assert_eq!(records[0]["payload"], expected_payload);
        assert!(
            records
                .iter()
                .all(|record| record["topic"] != "wave.worker.failed"),
            "partial timeout should not synthesize worker failures: {records:?}"
        );

        emit_wave_validation_marker(marker_id, &["backend", "error"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_text_backend() {
        let completed = run_wave_for_backend(
            BackendOutputFormat::Text,
            r#"printf 'plain text from worker\n'
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{"topic":"review.done","payload":"text backend ok","ts":"2026-01-01T00:00:00Z"}
EOF"#,
        )
        .await;

        assert_single_success_marked(&completed, "text backend ok", "output-format:text");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_claude_stream_json_backend() {
        let completed = run_wave_for_backend(
            BackendOutputFormat::StreamJson,
            r#"printf '%s\n' \
'{"type":"assistant","message":{"content":[{"type":"text","text":"hello from claude stream"}]}}' \
'{"type":"result","duration_ms":1,"total_cost_usd":0.0,"num_turns":1,"is_error":false}'
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{"topic":"review.done","payload":"claude stream ok","ts":"2026-01-01T00:00:00Z"}
EOF"#,
        )
        .await;

        assert_single_success_marked(
            &completed,
            "claude stream ok",
            "output-format:claude-stream-json",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_pi_stream_json_backend() {
        let completed = run_wave_for_backend(
            BackendOutputFormat::PiStreamJson,
            r#"printf '%s\n' \
'{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"hello from pi"}}' \
'{"type":"tool_execution_start","toolCallId":"toolu_1","toolName":"bash","args":{"command":"echo hi"}}' \
'{"type":"tool_execution_end","toolCallId":"toolu_1","toolName":"bash","result":{"content":[{"type":"text","text":"hi\n"}]},"isError":false}' \
'{"type":"turn_end","message":{"usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.0}}}}'
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{"topic":"review.done","payload":"pi stream ok","ts":"2026-01-01T00:00:00Z"}
EOF"#,
        )
        .await;

        assert_single_success_marked(&completed, "pi stream ok", "output-format:pi-stream-json");
    }

    #[cfg(unix)]
    named_text_wave_backend_test!(
        test_execute_wave_supports_named_kiro_backend,
        "kiro",
        "kiro backend ok"
    );

    #[cfg(unix)]
    named_text_wave_backend_test!(
        test_execute_wave_supports_named_gemini_backend,
        "gemini",
        "gemini backend ok"
    );

    #[cfg(unix)]
    named_text_wave_backend_test!(
        test_execute_wave_supports_named_codex_backend,
        "codex",
        "codex backend ok"
    );

    #[cfg(unix)]
    named_text_wave_backend_test!(
        test_execute_wave_supports_named_amp_backend,
        "amp",
        "amp backend ok"
    );

    #[cfg(unix)]
    named_text_wave_backend_test!(
        test_execute_wave_supports_named_copilot_backend,
        "copilot",
        "copilot backend ok"
    );

    #[cfg(unix)]
    named_text_wave_backend_test!(
        test_execute_wave_supports_named_opencode_backend,
        "opencode",
        "opencode backend ok"
    );

    #[cfg(unix)]
    named_text_wave_backend_test!(
        test_execute_wave_supports_named_roo_backend,
        "roo",
        "roo backend ok"
    );

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_named_claude_backend() {
        let completed =
            run_wave_for_named_backend("claude", &claude_backend_body("claude backend ok")).await;
        assert_single_success_marked(&completed, "claude backend ok", "named-backend:claude");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_named_pi_backend() {
        let completed = run_wave_for_named_backend("pi", &pi_backend_body("pi backend ok")).await;
        assert_single_success_marked(&completed, "pi backend ok", "named-backend:pi");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_named_kiro_acp_backend() {
        let _mock = install_mock_acp_executions(vec![MockAcpExecution::success(
            true,
            vec![make_worker_event("review.done", "kiro-acp backend ok")],
        )]);

        let completed = run_wave_for_named_backend("kiro-acp", &text_backend_body("unused")).await;
        assert_single_success_marked(&completed, "kiro-acp backend ok", "named-backend:kiro-acp");
    }

    #[cfg(unix)]
    fn large_wave_task_payload() -> String {
        format!(
            "ROLE: Validate this backend\n{}",
            "large-temp-file-wave-payload ".repeat(320)
        )
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_named_backend_invocation_contracts() {
        struct NamedBackendInvocationCase {
            name: &'static str,
            success_payload: &'static str,
            expected_prefix: &'static [&'static str],
            prompt_delivery: PromptDeliveryExpectation,
            marker_id: &'static str,
        }

        for case in [
            NamedBackendInvocationCase {
                name: "claude",
                success_payload: "claude invocation contract ok",
                expected_prefix: &[
                    "--dangerously-skip-permissions",
                    "--verbose",
                    "--output-format",
                    "stream-json",
                    "--print",
                    "--disallowedTools=TodoWrite,TaskCreate,TaskUpdate,TaskList,TaskGet",
                ],
                prompt_delivery: PromptDeliveryExpectation::Stdin,
                marker_id: "invocation-contract:named:claude",
            },
            NamedBackendInvocationCase {
                name: "pi",
                success_payload: "pi invocation contract ok",
                expected_prefix: &["-p", "--mode", "json", "--no-session"],
                prompt_delivery: PromptDeliveryExpectation::Positional,
                marker_id: "invocation-contract:named:pi",
            },
            NamedBackendInvocationCase {
                name: "kiro",
                success_payload: "kiro invocation contract ok",
                expected_prefix: &["chat", "--no-interactive", "--trust-all-tools"],
                prompt_delivery: PromptDeliveryExpectation::Positional,
                marker_id: "invocation-contract:named:kiro",
            },
            NamedBackendInvocationCase {
                name: "gemini",
                success_payload: "gemini invocation contract ok",
                expected_prefix: &["--yolo"],
                prompt_delivery: PromptDeliveryExpectation::Flag("-p"),
                marker_id: "invocation-contract:named:gemini",
            },
            NamedBackendInvocationCase {
                name: "codex",
                success_payload: "codex invocation contract ok",
                expected_prefix: &["exec", "--yolo"],
                prompt_delivery: PromptDeliveryExpectation::Positional,
                marker_id: "invocation-contract:named:codex",
            },
            NamedBackendInvocationCase {
                name: "amp",
                success_payload: "amp invocation contract ok",
                expected_prefix: &["--dangerously-allow-all"],
                prompt_delivery: PromptDeliveryExpectation::Flag("-x"),
                marker_id: "invocation-contract:named:amp",
            },
            NamedBackendInvocationCase {
                name: "copilot",
                success_payload: "copilot invocation contract ok",
                expected_prefix: &["--allow-all-tools"],
                prompt_delivery: PromptDeliveryExpectation::Flag("-p"),
                marker_id: "invocation-contract:named:copilot",
            },
            NamedBackendInvocationCase {
                name: "opencode",
                success_payload: "opencode invocation contract ok",
                expected_prefix: &["run"],
                prompt_delivery: PromptDeliveryExpectation::Positional,
                marker_id: "invocation-contract:named:opencode",
            },
            NamedBackendInvocationCase {
                name: "roo",
                success_payload: "roo invocation contract ok",
                expected_prefix: &["--print", "--ephemeral"],
                prompt_delivery: PromptDeliveryExpectation::PromptFile,
                marker_id: "invocation-contract:named:roo",
            },
        ] {
            let (completed, captured) =
                run_wave_for_named_backend_with_capture(case.name, case.success_payload).await;
            assert_single_success(&completed, case.success_payload);
            assert_named_backend_invocation_contract(
                &captured,
                case.expected_prefix,
                case.prompt_delivery,
            );
            emit_wave_validation_marker(case.marker_id, &["backend"]);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_named_backend_large_prompt_contracts() {
        struct NamedBackendLargePromptCase {
            name: &'static str,
            success_payload: &'static str,
            expected_prefix: &'static [&'static str],
            prompt_delivery: PromptDeliveryExpectation,
            marker_id: &'static str,
        }

        let task_payload = large_wave_task_payload();
        assert!(task_payload.len() > 7000, "expected large task payload");

        for case in [
            NamedBackendLargePromptCase {
                name: "claude",
                success_payload: "claude large prompt contract ok",
                expected_prefix: &[
                    "--dangerously-skip-permissions",
                    "--verbose",
                    "--output-format",
                    "stream-json",
                    "--print",
                    "--disallowedTools=TodoWrite,TaskCreate,TaskUpdate,TaskList,TaskGet",
                ],
                prompt_delivery: PromptDeliveryExpectation::Stdin,
                marker_id: "large-prompt-contract:named:claude",
            },
            NamedBackendLargePromptCase {
                name: "pi",
                success_payload: "pi large prompt contract ok",
                expected_prefix: &["-p", "--mode", "json", "--no-session"],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                marker_id: "large-prompt-contract:named:pi",
            },
            NamedBackendLargePromptCase {
                name: "kiro",
                success_payload: "kiro large prompt contract ok",
                expected_prefix: &["chat", "--no-interactive", "--trust-all-tools"],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                marker_id: "large-prompt-contract:named:kiro",
            },
            NamedBackendLargePromptCase {
                name: "gemini",
                success_payload: "gemini large prompt contract ok",
                expected_prefix: &["--yolo"],
                prompt_delivery: PromptDeliveryExpectation::TempFileFlag("-p"),
                marker_id: "large-prompt-contract:named:gemini",
            },
            NamedBackendLargePromptCase {
                name: "codex",
                success_payload: "codex large prompt contract ok",
                expected_prefix: &["exec", "--yolo"],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                marker_id: "large-prompt-contract:named:codex",
            },
            NamedBackendLargePromptCase {
                name: "amp",
                success_payload: "amp large prompt contract ok",
                expected_prefix: &["--dangerously-allow-all"],
                prompt_delivery: PromptDeliveryExpectation::TempFileFlag("-x"),
                marker_id: "large-prompt-contract:named:amp",
            },
            NamedBackendLargePromptCase {
                name: "copilot",
                success_payload: "copilot large prompt contract ok",
                expected_prefix: &["--allow-all-tools"],
                prompt_delivery: PromptDeliveryExpectation::TempFileFlag("-p"),
                marker_id: "large-prompt-contract:named:copilot",
            },
            NamedBackendLargePromptCase {
                name: "opencode",
                success_payload: "opencode large prompt contract ok",
                expected_prefix: &["run"],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                marker_id: "large-prompt-contract:named:opencode",
            },
        ] {
            let (completed, captured) = run_wave_for_named_backend_with_capture_and_task_payload(
                case.name,
                case.success_payload,
                &task_payload,
            )
            .await;
            assert_single_success(&completed, case.success_payload);
            assert_named_backend_invocation_contract(
                &captured,
                case.expected_prefix,
                case.prompt_delivery,
            );
            assert!(
                captured.prompt.contains(&task_payload),
                "captured prompt should include full large task payload for {}",
                case.name
            );
            emit_wave_validation_marker(case.marker_id, &["backend"]);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_hat_backend_invocation_contracts() {
        {
            let body = invocation_capture_backend_body("hat named invocation contract ok");
            let _fake = install_fake_path_backends(&[("gemini", body.as_str())]);
            let (completed, captured) = run_wave_for_hat_backend_with_capture(
                ralph_core::HatBackend::Named("gemini".to_string()),
                Some(vec!["--hat-runtime-arg".to_string()]),
            )
            .await;

            assert_single_success(&completed, "hat named invocation contract ok");
            assert_named_backend_invocation_contract(
                &captured,
                &["--yolo", "--hat-runtime-arg"],
                PromptDeliveryExpectation::Flag("-p"),
            );
            emit_wave_validation_marker("invocation-contract:hat:named", &["backend"]);
        }

        {
            let body =
                invocation_capture_backend_body("hat named-with-args invocation contract ok");
            let _fake = install_fake_path_backends(&[("opencode", body.as_str())]);
            let (completed, captured) = run_wave_for_hat_backend_with_capture(
                ralph_core::HatBackend::NamedWithArgs {
                    backend_type: "opencode".to_string(),
                    args: vec!["--from-hat-backend".to_string()],
                },
                Some(vec!["--hat-runtime-arg".to_string()]),
            )
            .await;

            assert_single_success(&completed, "hat named-with-args invocation contract ok");
            assert_named_backend_invocation_contract(
                &captured,
                &["run", "--from-hat-backend", "--hat-runtime-arg"],
                PromptDeliveryExpectation::Positional,
            );
            emit_wave_validation_marker("invocation-contract:hat:named-with-args", &["backend"]);
        }

        {
            struct HatNamedWithArgsInvocationCase {
                backend_type: &'static str,
                executable_name: &'static str,
                extra_args: &'static [&'static str],
                expected_prefix: &'static [&'static str],
                prompt_delivery: PromptDeliveryExpectation,
                success_payload: &'static str,
                marker_id: &'static str,
            }

            for case in [
                HatNamedWithArgsInvocationCase {
                    backend_type: "claude",
                    executable_name: "claude",
                    extra_args: &["--model", "claude-sonnet-4"],
                    expected_prefix: &[
                        "--dangerously-skip-permissions",
                        "--verbose",
                        "--output-format",
                        "stream-json",
                        "--print",
                        "--disallowedTools=TodoWrite,TaskCreate,TaskUpdate,TaskList,TaskGet",
                        "--model",
                        "claude-sonnet-4",
                        "--hat-runtime-arg",
                    ],
                    prompt_delivery: PromptDeliveryExpectation::Stdin,
                    success_payload: "hat claude named-with-args invocation contract ok",
                    marker_id: "invocation-contract:hat:named-with-args:claude",
                },
                HatNamedWithArgsInvocationCase {
                    backend_type: "pi",
                    executable_name: "pi",
                    extra_args: &["--provider", "anthropic", "--model", "claude-sonnet-4"],
                    expected_prefix: &[
                        "-p",
                        "--mode",
                        "json",
                        "--no-session",
                        "--provider",
                        "anthropic",
                        "--model",
                        "claude-sonnet-4",
                        "--hat-runtime-arg",
                    ],
                    prompt_delivery: PromptDeliveryExpectation::Positional,
                    success_payload: "hat pi named-with-args invocation contract ok",
                    marker_id: "invocation-contract:hat:named-with-args:pi",
                },
                HatNamedWithArgsInvocationCase {
                    backend_type: "kiro",
                    executable_name: "kiro-cli",
                    extra_args: &["--profile", "reviewer"],
                    expected_prefix: &[
                        "chat",
                        "--no-interactive",
                        "--trust-all-tools",
                        "--profile",
                        "reviewer",
                        "--hat-runtime-arg",
                    ],
                    prompt_delivery: PromptDeliveryExpectation::Positional,
                    success_payload: "hat kiro named-with-args invocation contract ok",
                    marker_id: "invocation-contract:hat:named-with-args:kiro",
                },
                HatNamedWithArgsInvocationCase {
                    backend_type: "gemini",
                    executable_name: "gemini",
                    extra_args: &["--model", "gemini-2.5-pro"],
                    expected_prefix: &["--yolo", "--model", "gemini-2.5-pro", "--hat-runtime-arg"],
                    prompt_delivery: PromptDeliveryExpectation::Flag("-p"),
                    success_payload: "hat gemini named-with-args invocation contract ok",
                    marker_id: "invocation-contract:hat:named-with-args:gemini",
                },
                HatNamedWithArgsInvocationCase {
                    backend_type: "codex",
                    executable_name: "codex",
                    extra_args: &["--dangerously-bypass-approvals-and-sandbox"],
                    expected_prefix: &["exec", "--yolo", "--hat-runtime-arg"],
                    prompt_delivery: PromptDeliveryExpectation::Positional,
                    success_payload: "hat codex named-with-args invocation contract ok",
                    marker_id: "invocation-contract:hat:named-with-args:codex",
                },
                HatNamedWithArgsInvocationCase {
                    backend_type: "amp",
                    executable_name: "amp",
                    extra_args: &["--model", "gpt-5"],
                    expected_prefix: &[
                        "--dangerously-allow-all",
                        "--model",
                        "gpt-5",
                        "--hat-runtime-arg",
                    ],
                    prompt_delivery: PromptDeliveryExpectation::Flag("-x"),
                    success_payload: "hat amp named-with-args invocation contract ok",
                    marker_id: "invocation-contract:hat:named-with-args:amp",
                },
                HatNamedWithArgsInvocationCase {
                    backend_type: "copilot",
                    executable_name: "copilot",
                    extra_args: &["--model", "gpt-5"],
                    expected_prefix: &[
                        "--allow-all-tools",
                        "--model",
                        "gpt-5",
                        "--hat-runtime-arg",
                    ],
                    prompt_delivery: PromptDeliveryExpectation::Flag("-p"),
                    success_payload: "hat copilot named-with-args invocation contract ok",
                    marker_id: "invocation-contract:hat:named-with-args:copilot",
                },
                HatNamedWithArgsInvocationCase {
                    backend_type: "roo",
                    executable_name: "roo",
                    extra_args: &["--model", "claude-sonnet-4"],
                    expected_prefix: &[
                        "--print",
                        "--ephemeral",
                        "--model",
                        "claude-sonnet-4",
                        "--hat-runtime-arg",
                    ],
                    prompt_delivery: PromptDeliveryExpectation::PromptFile,
                    success_payload: "hat roo named-with-args invocation contract ok",
                    marker_id: "invocation-contract:hat:named-with-args:roo",
                },
            ] {
                let body = invocation_capture_backend_body(case.success_payload);
                let _fake = install_fake_path_backends(&[(case.executable_name, body.as_str())]);
                let (completed, captured) = run_wave_for_hat_backend_with_capture(
                    ralph_core::HatBackend::NamedWithArgs {
                        backend_type: case.backend_type.to_string(),
                        args: case
                            .extra_args
                            .iter()
                            .map(|arg| (*arg).to_string())
                            .collect(),
                    },
                    Some(vec!["--hat-runtime-arg".to_string()]),
                )
                .await;

                assert_single_success(&completed, case.success_payload);
                assert_named_backend_invocation_contract(
                    &captured,
                    case.expected_prefix,
                    case.prompt_delivery,
                );
                emit_wave_validation_marker(case.marker_id, &["backend"]);
            }
        }

        {
            let body = invocation_capture_backend_body("hat kiro agent invocation contract ok");
            let _fake = install_fake_path_backends(&[("kiro-cli", body.as_str())]);
            let (completed, captured) = run_wave_for_hat_backend_with_capture(
                ralph_core::HatBackend::KiroAgent {
                    backend_type: "kiro".to_string(),
                    agent: "reviewer-agent".to_string(),
                    args: vec!["--kiro-extra".to_string()],
                },
                Some(vec!["--hat-runtime-arg".to_string()]),
            )
            .await;

            assert_single_success(&completed, "hat kiro agent invocation contract ok");
            assert_named_backend_invocation_contract(
                &captured,
                &[
                    "chat",
                    "--no-interactive",
                    "--trust-all-tools",
                    "--agent",
                    "reviewer-agent",
                    "--kiro-extra",
                    "--hat-runtime-arg",
                ],
                PromptDeliveryExpectation::Positional,
            );
            emit_wave_validation_marker("invocation-contract:hat:kiro-agent", &["backend"]);
        }

        {
            let temp_dir = tempfile::tempdir().expect("temp dir");
            let body = invocation_capture_backend_body("hat custom invocation contract ok");
            let worker_path = write_fake_executable(temp_dir.path(), "custom-wave-worker", &body);
            let (completed, captured) = run_wave_for_hat_backend_with_capture(
                ralph_core::HatBackend::Custom {
                    command: worker_path.display().to_string(),
                    args: vec!["--from-custom-backend".to_string()],
                },
                Some(vec!["--hat-runtime-arg".to_string()]),
            )
            .await;

            assert_single_success(&completed, "hat custom invocation contract ok");
            assert_named_backend_invocation_contract(
                &captured,
                &["--from-custom-backend", "--hat-runtime-arg"],
                PromptDeliveryExpectation::Positional,
            );
            emit_wave_validation_marker("invocation-contract:hat:custom", &["backend"]);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_hat_backend_large_prompt_contracts() {
        struct HatNamedLargePromptCase {
            backend_type: &'static str,
            executable_name: &'static str,
            expected_prefix: &'static [&'static str],
            prompt_delivery: PromptDeliveryExpectation,
            success_payload: &'static str,
            marker_id: &'static str,
        }

        struct HatNamedWithArgsLargePromptCase {
            backend_type: &'static str,
            executable_name: &'static str,
            extra_args: &'static [&'static str],
            expected_prefix: &'static [&'static str],
            prompt_delivery: PromptDeliveryExpectation,
            success_payload: &'static str,
            marker_id: &'static str,
        }

        let task_payload = large_wave_task_payload();
        assert!(task_payload.len() > 7000, "expected large task payload");

        for case in [
            HatNamedLargePromptCase {
                backend_type: "claude",
                executable_name: "claude",
                expected_prefix: &[
                    "--dangerously-skip-permissions",
                    "--verbose",
                    "--output-format",
                    "stream-json",
                    "--print",
                    "--disallowedTools=TodoWrite,TaskCreate,TaskUpdate,TaskList,TaskGet",
                    "--hat-runtime-arg",
                ],
                prompt_delivery: PromptDeliveryExpectation::Stdin,
                success_payload: "hat claude named large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named:claude",
            },
            HatNamedLargePromptCase {
                backend_type: "pi",
                executable_name: "pi",
                expected_prefix: &["-p", "--mode", "json", "--no-session", "--hat-runtime-arg"],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                success_payload: "hat pi named large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named:pi",
            },
            HatNamedLargePromptCase {
                backend_type: "kiro",
                executable_name: "kiro-cli",
                expected_prefix: &[
                    "chat",
                    "--no-interactive",
                    "--trust-all-tools",
                    "--hat-runtime-arg",
                ],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                success_payload: "hat kiro named large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named:kiro",
            },
            HatNamedLargePromptCase {
                backend_type: "gemini",
                executable_name: "gemini",
                expected_prefix: &["--yolo", "--hat-runtime-arg"],
                prompt_delivery: PromptDeliveryExpectation::TempFileFlag("-p"),
                success_payload: "hat gemini named large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named:gemini",
            },
            HatNamedLargePromptCase {
                backend_type: "codex",
                executable_name: "codex",
                expected_prefix: &["exec", "--yolo", "--hat-runtime-arg"],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                success_payload: "hat codex named large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named:codex",
            },
            HatNamedLargePromptCase {
                backend_type: "amp",
                executable_name: "amp",
                expected_prefix: &["--dangerously-allow-all", "--hat-runtime-arg"],
                prompt_delivery: PromptDeliveryExpectation::TempFileFlag("-x"),
                success_payload: "hat amp named large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named:amp",
            },
            HatNamedLargePromptCase {
                backend_type: "copilot",
                executable_name: "copilot",
                expected_prefix: &["--allow-all-tools", "--hat-runtime-arg"],
                prompt_delivery: PromptDeliveryExpectation::TempFileFlag("-p"),
                success_payload: "hat copilot named large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named:copilot",
            },
            HatNamedLargePromptCase {
                backend_type: "opencode",
                executable_name: "opencode",
                expected_prefix: &["run", "--hat-runtime-arg"],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                success_payload: "hat opencode named large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named:opencode",
            },
            HatNamedLargePromptCase {
                backend_type: "roo",
                executable_name: "roo",
                expected_prefix: &["--print", "--ephemeral", "--hat-runtime-arg"],
                prompt_delivery: PromptDeliveryExpectation::PromptFile,
                success_payload: "hat roo named large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named:roo",
            },
        ] {
            let body = invocation_capture_backend_body(case.success_payload);
            let _fake = install_fake_path_backends(&[(case.executable_name, body.as_str())]);
            let (completed, captured) = run_wave_for_hat_backend_with_capture_and_task_payload(
                ralph_core::HatBackend::Named(case.backend_type.to_string()),
                Some(vec!["--hat-runtime-arg".to_string()]),
                &task_payload,
            )
            .await;

            assert_single_success(&completed, case.success_payload);
            assert_named_backend_invocation_contract(
                &captured,
                case.expected_prefix,
                case.prompt_delivery,
            );
            assert!(
                captured.prompt.contains(&task_payload),
                "captured prompt should include full large task payload for {}",
                case.backend_type
            );
            emit_wave_validation_marker(case.marker_id, &["backend"]);
        }

        {
            let (completed, captured) = run_wave_for_hat_backend_with_acp_capture_and_task_payload(
                ralph_core::HatBackend::Named("kiro-acp".to_string()),
                Some(vec!["--hat-runtime-arg".to_string()]),
                "hat kiro-acp named large prompt contract ok",
                &task_payload,
            )
            .await;

            assert_single_success(&completed, "hat kiro-acp named large prompt contract ok");
            assert_acp_invocation_contract(&captured, &["acp", "--hat-runtime-arg"]);
            assert!(
                captured.prompt.contains(&task_payload),
                "captured prompt should include full large task payload for kiro-acp named hat"
            );
            emit_wave_validation_marker("large-prompt-contract:hat:named:kiro-acp", &["backend"]);
        }

        for case in [
            HatNamedWithArgsLargePromptCase {
                backend_type: "claude",
                executable_name: "claude",
                extra_args: &["--model", "claude-sonnet-4"],
                expected_prefix: &[
                    "--dangerously-skip-permissions",
                    "--verbose",
                    "--output-format",
                    "stream-json",
                    "--print",
                    "--disallowedTools=TodoWrite,TaskCreate,TaskUpdate,TaskList,TaskGet",
                    "--model",
                    "claude-sonnet-4",
                    "--hat-runtime-arg",
                ],
                prompt_delivery: PromptDeliveryExpectation::Stdin,
                success_payload: "hat claude named-with-args large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named-with-args:claude",
            },
            HatNamedWithArgsLargePromptCase {
                backend_type: "pi",
                executable_name: "pi",
                extra_args: &["--provider", "anthropic", "--model", "claude-sonnet-4"],
                expected_prefix: &[
                    "-p",
                    "--mode",
                    "json",
                    "--no-session",
                    "--provider",
                    "anthropic",
                    "--model",
                    "claude-sonnet-4",
                    "--hat-runtime-arg",
                ],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                success_payload: "hat pi named-with-args large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named-with-args:pi",
            },
            HatNamedWithArgsLargePromptCase {
                backend_type: "kiro",
                executable_name: "kiro-cli",
                extra_args: &["--profile", "reviewer"],
                expected_prefix: &[
                    "chat",
                    "--no-interactive",
                    "--trust-all-tools",
                    "--profile",
                    "reviewer",
                    "--hat-runtime-arg",
                ],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                success_payload: "hat kiro named-with-args large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named-with-args:kiro",
            },
            HatNamedWithArgsLargePromptCase {
                backend_type: "gemini",
                executable_name: "gemini",
                extra_args: &["--model", "gemini-2.5-pro"],
                expected_prefix: &["--yolo", "--model", "gemini-2.5-pro", "--hat-runtime-arg"],
                prompt_delivery: PromptDeliveryExpectation::TempFileFlag("-p"),
                success_payload: "hat gemini named-with-args large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named-with-args:gemini",
            },
            HatNamedWithArgsLargePromptCase {
                backend_type: "codex",
                executable_name: "codex",
                extra_args: &["--dangerously-bypass-approvals-and-sandbox"],
                expected_prefix: &["exec", "--yolo", "--hat-runtime-arg"],
                prompt_delivery: PromptDeliveryExpectation::TempFilePositional,
                success_payload: "hat codex named-with-args large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named-with-args:codex",
            },
            HatNamedWithArgsLargePromptCase {
                backend_type: "amp",
                executable_name: "amp",
                extra_args: &["--model", "gpt-5"],
                expected_prefix: &[
                    "--dangerously-allow-all",
                    "--model",
                    "gpt-5",
                    "--hat-runtime-arg",
                ],
                prompt_delivery: PromptDeliveryExpectation::TempFileFlag("-x"),
                success_payload: "hat amp named-with-args large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named-with-args:amp",
            },
            HatNamedWithArgsLargePromptCase {
                backend_type: "copilot",
                executable_name: "copilot",
                extra_args: &["--model", "gpt-5"],
                expected_prefix: &["--allow-all-tools", "--model", "gpt-5", "--hat-runtime-arg"],
                prompt_delivery: PromptDeliveryExpectation::TempFileFlag("-p"),
                success_payload: "hat copilot named-with-args large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named-with-args:copilot",
            },
            HatNamedWithArgsLargePromptCase {
                backend_type: "roo",
                executable_name: "roo",
                extra_args: &["--model", "claude-sonnet-4"],
                expected_prefix: &[
                    "--print",
                    "--ephemeral",
                    "--model",
                    "claude-sonnet-4",
                    "--hat-runtime-arg",
                ],
                prompt_delivery: PromptDeliveryExpectation::PromptFile,
                success_payload: "hat roo named-with-args large prompt contract ok",
                marker_id: "large-prompt-contract:hat:named-with-args:roo",
            },
        ] {
            let body = invocation_capture_backend_body(case.success_payload);
            let _fake = install_fake_path_backends(&[(case.executable_name, body.as_str())]);
            let (completed, captured) = run_wave_for_hat_backend_with_capture_and_task_payload(
                ralph_core::HatBackend::NamedWithArgs {
                    backend_type: case.backend_type.to_string(),
                    args: case
                        .extra_args
                        .iter()
                        .map(|arg| (*arg).to_string())
                        .collect(),
                },
                Some(vec!["--hat-runtime-arg".to_string()]),
                &task_payload,
            )
            .await;

            assert_single_success(&completed, case.success_payload);
            assert_named_backend_invocation_contract(
                &captured,
                case.expected_prefix,
                case.prompt_delivery,
            );
            assert!(
                captured.prompt.contains(&task_payload),
                "captured prompt should include full large task payload for {}",
                case.backend_type
            );
            emit_wave_validation_marker(case.marker_id, &["backend"]);
        }

        {
            let body = invocation_capture_backend_body("hat kiro agent large prompt contract ok");
            let _fake = install_fake_path_backends(&[("kiro-cli", body.as_str())]);
            let (completed, captured) = run_wave_for_hat_backend_with_capture_and_task_payload(
                ralph_core::HatBackend::KiroAgent {
                    backend_type: "kiro".to_string(),
                    agent: "reviewer-agent".to_string(),
                    args: vec!["--kiro-extra".to_string()],
                },
                Some(vec!["--hat-runtime-arg".to_string()]),
                &task_payload,
            )
            .await;

            assert_single_success(&completed, "hat kiro agent large prompt contract ok");
            assert_named_backend_invocation_contract(
                &captured,
                &[
                    "chat",
                    "--no-interactive",
                    "--trust-all-tools",
                    "--agent",
                    "reviewer-agent",
                    "--kiro-extra",
                    "--hat-runtime-arg",
                ],
                PromptDeliveryExpectation::TempFilePositional,
            );
            assert!(
                captured.prompt.contains(&task_payload),
                "captured prompt should include full large task payload for kiro agent hat"
            );
            emit_wave_validation_marker("large-prompt-contract:hat:kiro-agent:kiro", &["backend"]);
        }

        {
            let (completed, captured) = run_wave_for_hat_backend_with_acp_capture_and_task_payload(
                ralph_core::HatBackend::KiroAgent {
                    backend_type: "kiro-acp".to_string(),
                    agent: "reviewer-agent".to_string(),
                    args: vec!["--ignored-extra".to_string()],
                },
                Some(vec!["--hat-runtime-arg".to_string()]),
                "hat kiro-acp agent large prompt contract ok",
                &task_payload,
            )
            .await;

            assert_single_success(&completed, "hat kiro-acp agent large prompt contract ok");
            assert_acp_invocation_contract(
                &captured,
                &["acp", "--agent", "reviewer-agent", "--hat-runtime-arg"],
            );
            assert!(
                captured.prompt.contains(&task_payload),
                "captured prompt should include full large task payload for kiro-acp agent hat"
            );
            emit_wave_validation_marker(
                "large-prompt-contract:hat:kiro-agent:kiro-acp",
                &["backend"],
            );
        }

        {
            let temp_dir = tempfile::tempdir().expect("temp dir");
            let body = invocation_capture_backend_body("hat custom large prompt contract ok");
            let worker_path = write_fake_executable(temp_dir.path(), "custom-wave-worker", &body);
            let (completed, captured) = run_wave_for_hat_backend_with_capture_and_task_payload(
                ralph_core::HatBackend::Custom {
                    command: worker_path.display().to_string(),
                    args: vec!["--from-custom-backend".to_string()],
                },
                Some(vec!["--hat-runtime-arg".to_string()]),
                &task_payload,
            )
            .await;

            assert_single_success(&completed, "hat custom large prompt contract ok");
            assert_named_backend_invocation_contract(
                &captured,
                &["--from-custom-backend", "--hat-runtime-arg"],
                PromptDeliveryExpectation::TempFilePositional,
            );
            assert!(
                captured.prompt.contains(&task_payload),
                "captured prompt should include full large task payload for custom backend"
            );
            emit_wave_validation_marker("large-prompt-contract:hat:custom", &["backend"]);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_acp_backend_invocation_contracts() {
        {
            let (completed, captured) = run_wave_for_named_acp_backend_with_capture(
                Some(vec!["--hat-runtime-arg".to_string()]),
                "named ACP invocation contract ok",
            )
            .await;

            assert_single_success(&completed, "named ACP invocation contract ok");
            assert_acp_invocation_contract(&captured, &["acp", "--hat-runtime-arg"]);
            emit_wave_validation_marker("invocation-contract:acp:named:kiro-acp", &["backend"]);
        }

        {
            let (completed, captured) = run_wave_for_hat_backend_with_acp_capture(
                ralph_core::HatBackend::KiroAgent {
                    backend_type: "kiro-acp".to_string(),
                    agent: "reviewer-agent".to_string(),
                    args: vec!["--ignored-extra".to_string()],
                },
                Some(vec!["--hat-runtime-arg".to_string()]),
                "hat ACP kiro-agent invocation contract ok",
            )
            .await;

            assert_single_success(&completed, "hat ACP kiro-agent invocation contract ok");
            assert_acp_invocation_contract(
                &captured,
                &["acp", "--agent", "reviewer-agent", "--hat-runtime-arg"],
            );
            emit_wave_validation_marker("invocation-contract:acp:hat:kiro-agent", &["backend"]);
        }

        {
            let (completed, captured) = run_wave_for_hat_backend_with_acp_capture(
                ralph_core::HatBackend::NamedWithArgs {
                    backend_type: "kiro-acp".to_string(),
                    args: vec!["--model".to_string(), "claude-sonnet-4".to_string()],
                },
                Some(vec!["--hat-runtime-arg".to_string()]),
                "hat ACP named-with-args invocation contract ok",
            )
            .await;

            assert_single_success(&completed, "hat ACP named-with-args invocation contract ok");
            assert_acp_invocation_contract(
                &captured,
                &["acp", "--model", "claude-sonnet-4", "--hat-runtime-arg"],
            );
            emit_wave_validation_marker(
                "invocation-contract:acp:hat:named-with-args",
                &["backend"],
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_surfaces_named_kiro_acp_executor_error_with_synthetic_events() {
        let _mock = install_mock_acp_executions(vec![MockAcpExecution::error(
            "mock acp executor exploded",
            vec![],
        )]);

        let completed = run_wave_for_named_backend("kiro-acp", &text_backend_body("unused")).await;

        assert_single_failure_with_synthetic_events_marked(
            &completed,
            "mock acp executor exploded",
            "acp:named-executor-error",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_surfaces_hat_kiro_acp_timeout_without_events_with_synthetic_events()
    {
        let _mock = install_mock_acp_executions(vec![MockAcpExecution::timeout(vec![])]);

        let completed = run_wave_for_hat_backend(
            ralph_core::HatBackend::KiroAgent {
                backend_type: "kiro-acp".to_string(),
                agent: "reviewer-agent".to_string(),
                args: vec!["--unused-extra".to_string()],
            },
            Some(vec!["--hat-runtime-arg".to_string()]),
            missing_global_wave_backend(),
        )
        .await;

        assert_single_failure_with_synthetic_events_marked(
            &completed,
            "Worker timed out after 30s without emitting events",
            "acp:hat-timeout-without-events",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_hat_backend_named_with_backend_args() {
        let _fake = install_fake_path_backends(&[(
            "gemini",
            r#"found_hat_arg=0
for arg in "$@"; do
  if [ "$arg" = "--hat-runtime-arg" ]; then
    found_hat_arg=1
  fi
done
if [ "$found_hat_arg" -ne 1 ]; then
  echo "missing --hat-runtime-arg: $*" >&2
  exit 1
fi
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{"topic":"review.done","payload":"hat named backend ok","ts":"2026-01-01T00:00:00Z"}
EOF"#,
        )]);

        let completed = run_wave_for_hat_backend(
            ralph_core::HatBackend::Named("gemini".to_string()),
            Some(vec!["--hat-runtime-arg".to_string()]),
            missing_global_wave_backend(),
        )
        .await;

        assert_single_success_marked(&completed, "hat named backend ok", "hat-backend:named");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_hat_backend_named_with_args_and_backend_args() {
        let _fake = install_fake_path_backends(&[(
            "opencode",
            r#"found_hat_backend_arg=0
found_hat_runtime_arg=0
for arg in "$@"; do
  if [ "$arg" = "--from-hat-backend" ]; then
    found_hat_backend_arg=1
  fi
  if [ "$arg" = "--hat-runtime-arg" ]; then
    found_hat_runtime_arg=1
  fi
done
if [ "$found_hat_backend_arg" -ne 1 ] || [ "$found_hat_runtime_arg" -ne 1 ]; then
  echo "missing expected args: $*" >&2
  exit 1
fi
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{"topic":"review.done","payload":"hat named-with-args backend ok","ts":"2026-01-01T00:00:00Z"}
EOF"#,
        )]);

        let completed = run_wave_for_hat_backend(
            ralph_core::HatBackend::NamedWithArgs {
                backend_type: "opencode".to_string(),
                args: vec!["--from-hat-backend".to_string()],
            },
            Some(vec!["--hat-runtime-arg".to_string()]),
            missing_global_wave_backend(),
        )
        .await;

        assert_single_success_marked(
            &completed,
            "hat named-with-args backend ok",
            "hat-backend:named-with-args",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_hat_backend_kiro_agent_with_backend_args() {
        let _fake = install_fake_path_backends(&[(
            "kiro-cli",
            r#"found_agent=0
found_kiro_arg=0
found_hat_runtime_arg=0
prev=''
for arg in "$@"; do
  if [ "$prev" = "--agent" ] && [ "$arg" = "reviewer-agent" ]; then
    found_agent=1
  fi
  if [ "$arg" = "--kiro-extra" ]; then
    found_kiro_arg=1
  fi
  if [ "$arg" = "--hat-runtime-arg" ]; then
    found_hat_runtime_arg=1
  fi
  prev="$arg"
done
if [ "$found_agent" -ne 1 ] || [ "$found_kiro_arg" -ne 1 ] || [ "$found_hat_runtime_arg" -ne 1 ]; then
  echo "missing expected kiro args: $*" >&2
  exit 1
fi
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{"topic":"review.done","payload":"hat kiro agent backend ok","ts":"2026-01-01T00:00:00Z"}
EOF"#,
        )]);

        let completed = run_wave_for_hat_backend(
            ralph_core::HatBackend::KiroAgent {
                backend_type: "kiro".to_string(),
                agent: "reviewer-agent".to_string(),
                args: vec!["--kiro-extra".to_string()],
            },
            Some(vec!["--hat-runtime-arg".to_string()]),
            missing_global_wave_backend(),
        )
        .await;

        assert_single_success_marked(
            &completed,
            "hat kiro agent backend ok",
            "hat-backend:kiro-agent",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_hat_backend_kiro_acp_agent() {
        let _mock = install_mock_acp_executions(vec![MockAcpExecution::success(
            true,
            vec![make_worker_event(
                "review.done",
                "hat kiro-acp agent backend ok",
            )],
        )]);

        let completed = run_wave_for_hat_backend(
            ralph_core::HatBackend::KiroAgent {
                backend_type: "kiro-acp".to_string(),
                agent: "reviewer-agent".to_string(),
                args: vec!["--unused-extra".to_string()],
            },
            Some(vec!["--hat-runtime-arg".to_string()]),
            missing_global_wave_backend(),
        )
        .await;

        assert_single_success_marked(
            &completed,
            "hat kiro-acp agent backend ok",
            "hat-backend:kiro-acp-agent",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_supports_custom_hat_backend_with_backend_args() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let worker_path = write_fake_executable(
            temp_dir.path(),
            "custom-wave-worker",
            r#"found_custom_arg=0
found_hat_runtime_arg=0
for arg in "$@"; do
  if [ "$arg" = "--from-custom-backend" ]; then
    found_custom_arg=1
  fi
  if [ "$arg" = "--hat-runtime-arg" ]; then
    found_hat_runtime_arg=1
  fi
done
if [ "$found_custom_arg" -ne 1 ] || [ "$found_hat_runtime_arg" -ne 1 ]; then
  echo "missing expected custom args: $*" >&2
  exit 1
fi
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{"topic":"review.done","payload":"hat custom backend ok","ts":"2026-01-01T00:00:00Z"}
EOF"#,
        );

        let completed = run_wave_for_hat_backend(
            ralph_core::HatBackend::Custom {
                command: worker_path.display().to_string(),
                args: vec!["--from-custom-backend".to_string()],
            },
            Some(vec!["--hat-runtime-arg".to_string()]),
            missing_global_wave_backend(),
        )
        .await;

        assert_single_success_marked(&completed, "hat custom backend ok", "hat-backend:custom");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_synthesizes_failure_events_for_missing_custom_hat_backend_command() {
        let completed = run_wave_for_hat_backend(
            ralph_core::HatBackend::Custom {
                command: "/definitely/missing-custom-wave-worker".to_string(),
                args: vec!["--from-custom-backend".to_string()],
            },
            Some(vec!["--hat-runtime-arg".to_string()]),
            missing_global_wave_backend(),
        )
        .await;

        assert_single_failure_with_synthetic_events_marked(
            &completed,
            "missing-custom-wave-worker",
            "hat-backend:custom-missing-command",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_synthesizes_failure_events_for_missing_text_backend_command() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let events_file = temp_dir.path().join("events.jsonl");
        let wave = make_test_wave(vec!["review.done".to_string()]);

        let completed = execute_wave(
            &wave,
            &missing_global_wave_backend(),
            &events_file,
            false,
            false,
            None,
            None,
        )
        .await
        .expect("wave execution");

        assert_single_failure_with_synthetic_events_marked(
            &completed,
            "missing-wave-worker",
            "execution-mode:pty-spawn-failure-visible",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_synthesizes_failure_events_for_pty_open_failure() {
        let completed = run_wave_for_backend_with_test_env(
            BackendOutputFormat::Text,
            &text_backend_body("unused"),
            30,
            vec![("RALPH_TEST_FORCE_PTY_OPEN_FAIL", "mock openpty exploded")],
        )
        .await;

        assert_single_failure_with_synthetic_events_marked(
            &completed,
            "mock openpty exploded",
            "pty:open-failure-visible",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_synthesizes_failure_events_for_pty_reader_failure() {
        let completed = run_wave_for_backend_with_test_env(
            BackendOutputFormat::Text,
            &text_backend_body("unused"),
            30,
            vec![(
                "RALPH_TEST_FORCE_PTY_READER_FAIL",
                "mock reader clone exploded",
            )],
        )
        .await;

        assert_single_failure_with_synthetic_events_marked(
            &completed,
            "mock reader clone exploded",
            "pty:reader-failure-visible",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_falls_back_to_global_backend_when_hat_backend_is_invalid() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let worker_path = write_fake_executable(
            temp_dir.path(),
            "wave-worker",
            r#"found_fallback_arg=0
for arg in "$@"; do
  if [ "$arg" = "--fallback-arg" ]; then
    found_fallback_arg=1
  fi
done
if [ "$found_fallback_arg" -ne 1 ]; then
  echo "missing --fallback-arg: $*" >&2
  exit 1
fi
cat <<'EOF' > "$RALPH_EVENTS_FILE"
{"topic":"review.done","payload":"hat backend fallback ok","ts":"2026-01-01T00:00:00Z"}
EOF"#,
        );
        let global_backend = CliBackend {
            command: worker_path.display().to_string(),
            args: vec![],
            prompt_mode: ralph_adapters::PromptMode::Arg,
            prompt_flag: None,
            output_format: BackendOutputFormat::Text,
            env_vars: vec![],
        };

        let completed = run_wave_for_hat_backend(
            ralph_core::HatBackend::Named("definitely-invalid-backend".to_string()),
            Some(vec!["--fallback-arg".to_string()]),
            global_backend,
        )
        .await;

        assert_single_success_marked(
            &completed,
            "hat backend fallback ok",
            "hat-backend:invalid-fallback",
        );
        emit_wave_validation_marker("hat-backend:invalid-fallback", &["error"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_run_wave_worker_acp_timeout_with_partial_events_keeps_events_visible() {
        let _mock =
            install_mock_acp_executions(vec![MockAcpExecution::timeout(vec![make_worker_event(
                "review.done",
                "partial acp result",
            )])]);
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let worker_events_path = temp_dir.path().join("wave-events.jsonl");
        let merged_events_path = temp_dir.path().join("events.jsonl");
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let backend = CliBackend::kiro_acp();

        let (_index, outcome) = run_wave_worker_acp(
            0,
            &backend,
            "prompt",
            &worker_events_path,
            Duration::from_millis(1),
            tx,
            None,
            None,
        )
        .await;

        let (events, _duration, success) =
            outcome.expect("partial ACP timeout should preserve emitted events");
        assert!(!success, "timed out worker should not be marked successful");
        assert_eq!(events.len(), 1, "unexpected partial events: {events:?}");
        assert_eq!(events[0].topic.as_str(), "review.done");
        assert_eq!(events[0].payload.as_deref(), Some("partial acp result"));

        let completed = ralph_core::CompletedWave {
            wave_id: "w-acp".to_string(),
            results: vec![ralph_core::WaveResult {
                index: 0,
                events: events.into_iter().map(ralph_proto::Event::from).collect(),
            }],
            failures: vec![],
            duration: Duration::from_millis(1),
        };

        merge_wave_results_to_events_file(
            &completed,
            &merged_events_path,
            &["review.done".to_string()],
        )
        .expect("merge partial ACP results");

        let merged = std::fs::read_to_string(&merged_events_path).expect("read merged events");
        let records: Vec<serde_json::Value> = merged
            .lines()
            .map(|line| serde_json::from_str(line).expect("json event"))
            .collect();

        assert_eq!(records.len(), 1, "unexpected merged records: {records:?}");
        assert_eq!(records[0]["topic"], "review.done");
        assert_eq!(records[0]["payload"], "partial acp result");
        assert!(
            records
                .iter()
                .all(|record| record["topic"] != "wave.worker.failed"),
            "partial ACP timeout should not synthesize worker failures: {records:?}"
        );
        emit_wave_validation_marker("acp:partial-timeout-visible-events", &["backend", "error"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_keeps_text_partial_timeout_events_visible() {
        let completed = run_wave_for_backend_with_timeout(
            BackendOutputFormat::Text,
            &body_with_post_event_sleep(text_backend_body("text partial timeout ok")),
            1,
        )
        .await;

        assert_partial_timeout_events_visible_marked(
            &completed,
            "text partial timeout ok",
            "pty:text-partial-timeout-visible-events",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_keeps_claude_stream_partial_timeout_events_visible() {
        let completed = run_wave_for_backend_with_timeout(
            BackendOutputFormat::StreamJson,
            &body_with_post_event_sleep(claude_backend_body("claude partial timeout ok")),
            1,
        )
        .await;

        assert_partial_timeout_events_visible_marked(
            &completed,
            "claude partial timeout ok",
            "pty:claude-stream-partial-timeout-visible-events",
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_wave_keeps_pi_stream_partial_timeout_events_visible() {
        let completed = run_wave_for_backend_with_timeout(
            BackendOutputFormat::PiStreamJson,
            &body_with_post_event_sleep(pi_backend_body("pi partial timeout ok")),
            1,
        )
        .await;

        assert_partial_timeout_events_visible_marked(
            &completed,
            "pi partial timeout ok",
            "pty:pi-stream-partial-timeout-visible-events",
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_wave_worker_execution_mode_uses_acp_for_kiro_acp_backend() {
        let backend = CliBackend::kiro_acp();
        assert_eq!(
            wave_worker_execution_mode(backend.output_format),
            WaveWorkerExecutionMode::Acp
        );
        emit_wave_validation_marker("execution-mode:kiro-acp-backend", &["backend"]);
    }

    #[cfg(unix)]
    #[test]
    fn test_wave_worker_execution_mode_uses_acp_for_kiro_acp_hat_backend() {
        let backend = CliBackend::from_hat_backend(&ralph_core::HatBackend::KiroAgent {
            backend_type: "kiro-acp".to_string(),
            agent: "reviewer".to_string(),
            args: vec![],
        })
        .expect("kiro-acp backend");
        assert_eq!(
            wave_worker_execution_mode(backend.output_format),
            WaveWorkerExecutionMode::Acp
        );
        emit_wave_validation_marker("execution-mode:kiro-acp-hat-backend", &["backend"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_run_wave_worker_pty_surfaces_spawn_failure() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let missing_cmd = temp_dir.path().join("missing-wave-worker");
        let worker_events_path = temp_dir.path().join("wave-events.jsonl");
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let backend = CliBackend {
            command: missing_cmd.display().to_string(),
            args: vec![],
            prompt_mode: ralph_adapters::PromptMode::Arg,
            prompt_flag: None,
            output_format: BackendOutputFormat::Text,
            env_vars: vec![],
        };

        let (_index, outcome) = run_wave_worker_pty(
            0,
            &backend,
            "prompt",
            &worker_events_path,
            Duration::from_secs(1),
            tx,
            None,
            None,
        )
        .await;

        let (error, _duration) = outcome.expect_err("missing worker should fail to spawn");
        assert!(
            error.contains("PTY spawn failed"),
            "unexpected error: {error}"
        );
        emit_wave_validation_marker("pty:spawn-failure", &["error"]);
    }

    #[cfg(unix)]
    #[test]
    fn test_merge_wave_results_to_events_file_synthesizes_failure_events() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let events_file = temp_dir.path().join("events.jsonl");
        let completed = ralph_core::CompletedWave {
            wave_id: "w-test".to_string(),
            results: vec![ralph_core::WaveResult {
                index: 0,
                events: vec![ralph_proto::Event::new("review.done", "worker ok")],
            }],
            failures: vec![ralph_core::WaveFailure {
                index: 1,
                error: "PTY spawn failed: missing-worker".to_string(),
                duration: Duration::from_secs(1),
            }],
            duration: Duration::from_secs(1),
        };

        merge_wave_results_to_events_file(
            &completed,
            &events_file,
            &["review.done".to_string(), "review.audit".to_string()],
        )
        .expect("merge wave results");

        let content = std::fs::read_to_string(&events_file).expect("read merged events");
        let records: Vec<serde_json::Value> = content
            .lines()
            .map(|line| serde_json::from_str(line).expect("json event"))
            .collect();

        assert_eq!(records.len(), 4, "unexpected merged records: {records:?}");
        assert!(records.iter().any(|record| {
            record["topic"] == "wave.worker.failed"
                && record["payload"]
                    .as_str()
                    .is_some_and(|payload| payload.contains("PTY spawn failed: missing-worker"))
                && record["wave_index"] == 1
        }));
        assert!(records.iter().any(|record| {
            record["topic"] == "review.done"
                && record["payload"]
                    .as_str()
                    .is_some_and(|payload| payload.contains("## Worker 1 (FAILED)"))
        }));
        assert!(records.iter().any(|record| {
            record["topic"] == "review.audit"
                && record["payload"].as_str().is_some_and(|payload| {
                    payload.contains("Error: PTY spawn failed: missing-worker")
                })
        }));
        emit_wave_validation_marker(
            "merge-wave-results:synthetic-failure-events",
            &["error", "synthetic"],
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

    #[cfg(unix)]
    #[test]
    fn test_process_pending_merges_redirects_subprocess_output_to_log_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();

        // Enqueue a merge entry using the proper API
        let queue = ralph_core::merge_queue::MergeQueue::new(repo_root);
        queue.enqueue("test-loop", "merge prompt").expect("enqueue");

        // Create a fake ralph that writes to both stdout and stderr
        let bin_dir = repo_root.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let ralph_path = write_fake_executable(
            &bin_dir,
            "ralph",
            "echo 'stdout output' && echo 'stderr output' >&2 && sleep 0.1",
        );

        process_pending_merges_with_command(repo_root, ralph_path.as_os_str());

        // Wait for subprocess to finish writing
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Verify a log file was created under .ralph/diagnostics/logs/
        let logs_dir = repo_root.join(".ralph/diagnostics/logs");
        assert!(logs_dir.exists(), "diagnostics logs directory should exist");

        let log_files: Vec<_> = std::fs::read_dir(&logs_dir)
            .expect("read logs dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("ralph-merge-"))
            .collect();
        assert!(
            !log_files.is_empty(),
            "should have at least one merge subprocess log file"
        );

        // Verify the log file contains the subprocess output
        let log_content = std::fs::read_to_string(log_files[0].path()).expect("read log file");
        assert!(
            log_content.contains("stdout output"),
            "log file should contain stdout, got: {log_content}"
        );
        assert!(
            log_content.contains("stderr output"),
            "log file should contain stderr, got: {log_content}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_process_pending_merges_falls_back_to_null_on_log_creation_failure() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();

        // Block log file creation by placing a regular file where the logs directory would be
        let diagnostics_dir = repo_root.join(".ralph/diagnostics");
        std::fs::create_dir_all(&diagnostics_dir).expect("diagnostics dir");
        std::fs::write(diagnostics_dir.join("logs"), "not a directory").expect("block logs dir");

        // Enqueue a merge entry using the proper API
        let queue = ralph_core::merge_queue::MergeQueue::new(repo_root);
        queue.enqueue("test-loop", "merge prompt").expect("enqueue");

        // Create a fake ralph
        let bin_dir = repo_root.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let ralph_path = write_fake_executable(&bin_dir, "ralph", "exit 0");

        // Should not panic even though log file creation fails
        process_pending_merges_with_command(repo_root, ralph_path.as_os_str());
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

    #[test]
    fn test_recover_late_events_before_fallback_routes_pending_work() {
        use std::io::Write;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let yaml = r#"
hats:
  investigator:
    name: "Investigator"
    triggers: ["debug.start", "hypothesis.rejected", "hypothesis.confirmed", "fix.verified"]
    publishes: ["hypothesis.test", "fix.propose", "DEBUG_COMPLETE"]
  tester:
    name: "Tester"
    triggers: ["hypothesis.test"]
    publishes: ["hypothesis.confirmed", "hypothesis.rejected"]
"#;
        let (mut event_loop, loop_ctx) =
            dispatch_test_event_loop_from_yaml_with_context(temp_dir.path(), yaml);
        let events_path = loop_ctx.events_path();
        std::fs::create_dir_all(events_path.parent().expect("events path parent"))
            .expect("create events directory");

        let mut events_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)
            .expect("open events file");
        writeln!(
            events_file,
            r#"{{"topic":"hypothesis.test","payload":"Race condition suspected","ts":"2026-03-08T00:00:00Z"}}"#
        )
        .expect("write late event");
        events_file.flush().expect("flush late event");

        let outcome =
            recover_late_events_before_fallback(&mut event_loop).expect("recover late events");
        assert_eq!(outcome, LateEventRecovery::PendingWork);
        assert_eq!(
            event_loop.next_hat().map(|hat| hat.as_str()),
            Some("ralph"),
            "late downstream work should route the next iteration to Ralph in multi-hat mode"
        );

        let tester_id = HatId::new("tester");
        let tester_pending = event_loop
            .bus()
            .peek_pending(&tester_id)
            .cloned()
            .unwrap_or_default();
        assert_eq!(tester_pending.len(), 1);
        assert_eq!(tester_pending[0].topic.as_str(), "hypothesis.test");
    }

    #[test]
    fn test_recover_late_events_before_fallback_honors_completion() {
        use std::io::Write;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let (mut event_loop, loop_ctx) = dispatch_test_event_loop_with_context(temp_dir.path());
        let events_path = loop_ctx.events_path();
        std::fs::create_dir_all(events_path.parent().expect("events path parent"))
            .expect("create events directory");

        let mut events_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)
            .expect("open events file");
        writeln!(
            events_file,
            r#"{{"topic":"LOOP_COMPLETE","payload":"done","ts":"2026-03-08T00:00:00Z"}}"#
        )
        .expect("write completion event");
        events_file.flush().expect("flush completion event");

        let outcome =
            recover_late_events_before_fallback(&mut event_loop).expect("recover completion");
        assert_eq!(
            outcome,
            LateEventRecovery::Terminate(TerminationReason::CompletionPromise)
        );
    }

    #[test]
    fn test_recover_late_events_before_fallback_polls_for_delayed_completion() {
        use std::io::Write;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let (mut event_loop, loop_ctx) = dispatch_test_event_loop_with_context(temp_dir.path());
        let events_path = loop_ctx.events_path();
        std::fs::create_dir_all(events_path.parent().expect("events path parent"))
            .expect("create events directory");

        let delayed_events_path = events_path.clone();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            let mut events_file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&delayed_events_path)
                .expect("open delayed events file");
            writeln!(
                events_file,
                r#"{{"topic":"LOOP_COMPLETE","payload":"done","ts":"2026-03-08T00:00:00Z"}}"#
            )
            .expect("write delayed completion event");
            events_file.flush().expect("flush delayed completion event");
        });

        let outcome =
            recover_late_events_before_fallback(&mut event_loop).expect("recover completion");
        writer.join().expect("join delayed event writer");

        assert_eq!(
            outcome,
            LateEventRecovery::Terminate(TerminationReason::CompletionPromise)
        );
    }

    #[test]
    fn test_recover_expected_emit_after_output_polls_for_delayed_completion() {
        use std::io::Write;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let (mut event_loop, loop_ctx) = dispatch_test_event_loop_with_context(temp_dir.path());
        let events_path = loop_ctx.events_path();
        std::fs::create_dir_all(events_path.parent().expect("events path parent"))
            .expect("create events directory");

        let delayed_events_path = events_path.clone();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            let mut events_file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&delayed_events_path)
                .expect("open delayed events file");
            writeln!(
                events_file,
                r#"{{"topic":"LOOP_COMPLETE","payload":"done","ts":"2026-03-08T00:00:00Z"}}"#
            )
            .expect("write delayed completion event");
            events_file.flush().expect("flush delayed completion event");
        });

        let outcome =
            recover_expected_emit_after_output(&mut event_loop).expect("recover expected emit");
        writer.join().expect("join delayed event writer");

        assert_eq!(
            outcome,
            LateEventRecovery::Terminate(TerminationReason::CompletionPromise)
        );
    }

    #[test]
    fn test_resolve_display_hat_for_execution_prefers_prompt_selected_hat_for_ralph() {
        let yaml = r#"
hats:
  investigator:
    name: "Investigator"
    triggers: ["debug.start", "hypothesis.confirmed"]
  tester:
    name: "Tester"
    triggers: ["hypothesis.test"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).expect("yaml config");
        let mut event_loop = EventLoop::new(config);

        event_loop
            .bus()
            .publish(Event::new("hypothesis.test", "Test the hypothesis"));
        event_loop
            .build_prompt(&HatId::new("ralph"))
            .expect("prompt should build");

        let display_hat = resolve_display_hat_for_execution(
            &event_loop,
            &HatId::new("ralph"),
            &HatId::new("investigator"),
        );

        assert_eq!(display_hat.as_str(), "tester");
    }

    #[test]
    fn test_resolve_display_hat_for_execution_ignores_targeted_task_resume_noise() {
        let yaml = r#"
hats:
  investigator:
    name: "Investigator"
    triggers: ["task.resume", "debug.start", "hypothesis.confirmed"]
  tester:
    name: "Tester"
    triggers: ["hypothesis.test"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).expect("yaml config");
        let mut event_loop = EventLoop::new(config);

        event_loop
            .bus()
            .publish(Event::new("task.resume", "Recovery").with_target("investigator"));
        event_loop
            .bus()
            .publish(Event::new("hypothesis.test", "Test the hypothesis"));
        event_loop
            .build_prompt(&HatId::new("ralph"))
            .expect("prompt should build");

        let display_hat = resolve_display_hat_for_execution(
            &event_loop,
            &HatId::new("ralph"),
            &HatId::new("investigator"),
        );

        assert_eq!(display_hat.as_str(), "tester");
    }

    #[test]
    fn test_resolve_display_hat_for_execution_prefers_downstream_event_over_start_event() {
        let yaml = r#"
hats:
  investigator:
    name: "Investigator"
    triggers: ["debug.start", "hypothesis.confirmed"]
  tester:
    name: "Tester"
    triggers: ["hypothesis.test"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).expect("yaml config");
        let mut event_loop = EventLoop::new(config);

        event_loop
            .bus()
            .publish(Event::new("debug.start", "Investigate the bug"));
        event_loop
            .bus()
            .publish(Event::new("hypothesis.test", "Test the hypothesis"));
        event_loop
            .build_prompt(&HatId::new("ralph"))
            .expect("prompt should build");

        let display_hat = resolve_display_hat_for_execution(
            &event_loop,
            &HatId::new("ralph"),
            &HatId::new("investigator"),
        );

        assert_eq!(display_hat.as_str(), "tester");
    }

    #[test]
    fn test_resolve_display_hat_for_execution_keeps_explicit_non_ralph_hat() {
        let event_loop = EventLoop::new(RalphConfig::default());

        let display_hat = resolve_display_hat_for_execution(
            &event_loop,
            &HatId::new("fixer"),
            &HatId::new("investigator"),
        );

        assert_eq!(display_hat.as_str(), "fixer");
    }

    #[test]
    fn test_output_mentions_ralph_emit_detects_tool_call_output() {
        assert!(output_mentions_ralph_emit(
            r#"[Tool] Bash: ralph emit "hypothesis.test" "payload""#
        ));
        assert!(!output_mentions_ralph_emit("[Tool] Bash: cargo test"));
    }
}
