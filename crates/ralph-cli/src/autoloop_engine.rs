//! v3 cutover: drive the autoloop runtime as ralph's orchestration engine.
//!
//! When `core.engine = "autoloop"`, `ralph run` spawns `autoloop run` as a
//! subprocess via [`AutoloopRunner`] instead of the in-house event loop,
//! consumes its structured `--events` LoopEvent stream, and maps the terminal
//! result onto ralph's [`TerminationReason`]. This is the thin-layer engine swap
//! at the heart of v3: autoloop owns loop execution; ralph coordinates.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ralph_adapters::{AutoloopRunner, events_run_result, parse_events};
use ralph_core::{RalphConfig, TerminationReason};

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
/// [`TerminationReason`]. Requires `core.autoloop_preset` to point at an autoloop
/// preset directory (containing `autoloops.toml`).
pub async fn run_autoloop_engine(config: RalphConfig) -> Result<TerminationReason> {
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

    let runner = AutoloopRunner::new(preset, prompt, workspace.clone())
        .events_path(events_path.clone());

    // AutoloopRunner::run blocks on the subprocess; keep the async runtime free.
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

    Ok(map_stop_reason(&summary.stop_reason))
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
