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
use ralph_adapters::{AutoloopRunner, events_run_result, parse_events};
use ralph_core::{LoopContext, RalphConfig, RunStats, TerminationReason};

use crate::completion_coord::coordinate_completion;

/// Map an autoloop `stopReason` onto ralph's [`TerminationReason`].
fn map_stop_reason(reason: &str) -> TerminationReason {
    match reason {
        "completed" | "verdict_exit" => TerminationReason::CompletionPromise,
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

    // AutoloopRunner::run blocks on the subprocess; keep the async runtime free.
    let start = Instant::now();
    let summary = tokio::task::spawn_blocking(move || runner.run())
        .await
        .context("autoloop run task panicked")?
        .context("autoloop run failed")?;

    if let Ok(content) = std::fs::read_to_string(&events_path) {
        if let Some(result) = events_run_result(&parse_events(&content)) {
            println!(
                "autoloop engine: run_id={} iterations={} stop_reason={}",
                result.run_id, result.iterations, result.stop_reason
            );
        }
    }

    let reason = map_stop_reason(&summary.stop_reason);

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

/// Start a headless orchestration loop for the Telegram bot daemon.
///
/// This is the daemon's [`ralph_proto::StartLoopFn`] target: it loads config,
/// applies the supplied prompt, forces autonomous/headless mode, acquires the
/// primary loop lock, and drives the autoloop engine.
///
/// Note: human-in-the-loop robot wiring under the autoloop engine is descoped
/// to #345. The loop still runs; in-loop Telegram interaction is not yet routed.
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

    // Drive the loop headlessly via the autoloop engine.
    run_autoloop_engine(config, Some(loop_context), None, None, false).await
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
}
