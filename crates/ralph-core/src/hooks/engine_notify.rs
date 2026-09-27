//! Lifecycle hooks under the autoloop engine.
//!
//! The engine owns the loop, so Ralph cannot run hooks between iterations.
//! The one seam it offers is its finish notification (`notify.command`, fired
//! once when the loop ends and journaled as `notify.sent` / `notify.failed`).
//! Ralph points that command at `ralph hooks notify`, which reads the engine's
//! finish payload and runs the configured hooks with Ralph's own lifecycle
//! payload, so existing hook scripts keep receiving the shape they were
//! written against.
//!
//! Only `post.loop.complete` and `post.loop.error` have that seam. Every other
//! event, mutating hooks, and `on_error: block|suspend` (which need a live loop
//! to act on) are refused before the run starts instead of silently ignored.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::engine::{HookEngine, HookPayloadBuilderInput, HookPayloadContextInput};
use super::executor::{HookExecutorContract, HookRunRequest, HookRunResult};
use crate::config::{HookOnError, HookPhaseEvent, HooksConfig};

/// A hooks configuration the engine cannot honor.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineHookError {
    #[error(
        "hook event `{event}` has no equivalent under the v3 autoloop engine; only post.loop.complete and post.loop.error are supported. Remove the `{event}` hooks or move them to one of those events."
    )]
    UnsupportedEvent { event: &'static str },
    #[error(
        "hook `{hook}` on `{event}` sets `mutate.enabled`, which the v3 autoloop engine cannot apply after the loop has finished. Remove `mutate` from that hook."
    )]
    Mutating { event: &'static str, hook: String },
    #[error(
        "hook `{hook}` on `{event}` sets `on_error: {on_error}`, which needs a running loop to block or suspend; under the v3 autoloop engine the hook fires after the loop ends. Use `on_error: warn`."
    )]
    OnError {
        event: &'static str,
        hook: String,
        on_error: &'static str,
    },
}

/// Engine stop-reason classes, mirroring autoloop's `classifyStopReason`.
const COMPLETED: &str = "completed";
const FAILED: &str = "failed";
const STOPPED: &str = "stopped";

/// Checks that every configured hook can run under the engine.
pub fn validate_engine_hooks(config: &HooksConfig) -> Result<(), EngineHookError> {
    if !config.enabled {
        return Ok(());
    }
    let mut events: Vec<_> = config
        .events
        .iter()
        .filter(|(_, specs)| !specs.is_empty())
        .collect();
    events.sort_by_key(|(event, _)| event.as_str());
    for (event, specs) in events {
        let name = event.as_str();
        if !matches!(
            event,
            HookPhaseEvent::PostLoopComplete | HookPhaseEvent::PostLoopError
        ) {
            return Err(EngineHookError::UnsupportedEvent { event: name });
        }
        for spec in specs {
            if spec.mutate.enabled {
                return Err(EngineHookError::Mutating {
                    event: name,
                    hook: spec.name.clone(),
                });
            }
            let on_error = match spec.on_error {
                Some(HookOnError::Block) => Some("block"),
                Some(HookOnError::Suspend) => Some("suspend"),
                Some(HookOnError::Warn) | None => None,
            };
            if let Some(on_error) = on_error {
                return Err(EngineHookError::OnError {
                    event: name,
                    hook: spec.name.clone(),
                    on_error,
                });
            }
        }
    }
    Ok(())
}

/// The engine `notify.on` classes that must fire, or `None` when no hook
/// needs the finish notification.
///
/// `post.loop.error` means "the loop ended without completing", so it covers
/// both the engine's `failed` and `stopped` classes (a budget or iteration
/// limit is a loop that did not complete).
pub fn notify_classes(config: &HooksConfig) -> Option<String> {
    if !config.enabled {
        return None;
    }
    let has = |event| {
        config
            .events
            .get(&event)
            .is_some_and(|specs| !specs.is_empty())
    };
    let mut classes = Vec::new();
    if has(HookPhaseEvent::PostLoopComplete) {
        classes.push(COMPLETED);
    }
    if has(HookPhaseEvent::PostLoopError) {
        classes.extend([FAILED, STOPPED]);
    }
    (!classes.is_empty()).then(|| classes.join(","))
}

/// The Ralph hook event for an engine stop reason.
pub fn phase_event_for_stop_reason(stop_reason: &str) -> HookPhaseEvent {
    if stop_reason == COMPLETED || stop_reason.starts_with("completion") {
        HookPhaseEvent::PostLoopComplete
    } else {
        HookPhaseEvent::PostLoopError
    }
}

/// What `ralph hooks notify` needs to rebuild Ralph's payload, captured when
/// the run starts so the hooks that fire are the hooks that were configured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifySnapshot {
    pub loop_id: String,
    pub is_primary: bool,
    pub workspace: PathBuf,
    pub repo_root: PathBuf,
    pub max_iterations: u32,
    pub hooks: HooksConfig,
}

impl NotifySnapshot {
    /// Where the snapshot lives beneath the engine state root.
    pub fn path(engine_state_root: &Path) -> PathBuf {
        engine_state_root.join("notify-hooks.json")
    }

    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    pub fn read(path: &Path) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(std::io::Error::other)
    }

    /// Upper bound on how long all hooks for either event may run, in ms,
    /// used for the engine's `notify.timeout_ms`.
    pub fn timeout_ms(&self) -> u64 {
        let engine = HookEngine::new(&self.hooks);
        [
            HookPhaseEvent::PostLoopComplete,
            HookPhaseEvent::PostLoopError,
        ]
        .into_iter()
        .map(|event| {
            engine
                .resolve_phase_event(event)
                .iter()
                .map(|hook| hook.timeout_seconds)
                .sum::<u64>()
        })
        .max()
        .unwrap_or(0)
        .saturating_mul(1_000)
        .saturating_add(5_000)
    }
}

/// The engine's finish-notification payload (stdin of `notify.command`).
#[derive(Debug, Clone, Deserialize)]
pub struct EngineFinishPayload {
    pub run_id: String,
    pub stop_reason: String,
    #[serde(default)]
    pub iterations: u32,
}

/// One hook's outcome.
#[derive(Debug)]
pub struct DispatchedHook {
    pub name: String,
    pub result: Result<HookRunResult, String>,
}

impl DispatchedHook {
    pub fn succeeded(&self) -> bool {
        matches!(&self.result, Ok(run) if run.exit_code == Some(0) && !run.timed_out)
    }
}

/// Runs the hooks for the event the engine's stop reason maps to, with
/// Ralph's lifecycle payload on stdin.
pub fn dispatch(
    snapshot: &NotifySnapshot,
    finish: &EngineFinishPayload,
    executor: &impl HookExecutorContract,
) -> (HookPhaseEvent, Vec<DispatchedHook>) {
    let event = phase_event_for_stop_reason(&finish.stop_reason);
    let engine = HookEngine::new(&snapshot.hooks);
    let hooks = if snapshot.hooks.enabled {
        engine.resolve_phase_event(event)
    } else {
        Vec::new()
    };
    let outcomes = hooks
        .into_iter()
        .map(|hook| {
            let payload = engine.build_payload(
                event,
                HookPayloadBuilderInput {
                    loop_id: snapshot.loop_id.clone(),
                    is_primary: snapshot.is_primary,
                    workspace: snapshot.workspace.clone(),
                    repo_root: snapshot.repo_root.clone(),
                    pid: std::process::id(),
                    iteration_current: finish.iterations,
                    iteration_max: snapshot.max_iterations,
                    context: HookPayloadContextInput {
                        termination_reason: Some(finish.stop_reason.clone()),
                        ..HookPayloadContextInput::default()
                    },
                },
            );
            let result = serde_json::to_value(&payload)
                .map_err(|error| error.to_string())
                .and_then(|stdin_payload| {
                    executor
                        .run(HookRunRequest {
                            phase_event: event.as_str().to_string(),
                            hook_name: hook.name.clone(),
                            command: hook.command.clone(),
                            workspace_root: snapshot.workspace.clone(),
                            cwd: hook.cwd.clone(),
                            env: hook.env.clone(),
                            // The engine's AUTOLOOP_* variables are not part of
                            // Ralph's hook contract; hooks written for Ralph
                            // must see the environment they always saw.
                            env_remove: std::env::vars()
                                .map(|(key, _)| key)
                                .filter(|key| key.starts_with("AUTOLOOP_"))
                                .collect(),
                            timeout_seconds: hook.timeout_seconds,
                            max_output_bytes: hook.max_output_bytes,
                            stdin_payload,
                        })
                        .map_err(|error| error.to_string())
                });
            DispatchedHook {
                name: hook.name,
                result,
            }
        })
        .collect();
    (event, outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HookMutationConfig, HookSpec};
    use std::collections::HashMap;
    use std::sync::Mutex;

    fn spec(name: &str, on_error: HookOnError) -> HookSpec {
        HookSpec {
            name: name.to_string(),
            command: vec!["notify".to_string()],
            cwd: None,
            env: HashMap::new(),
            timeout_seconds: Some(20),
            max_output_bytes: None,
            on_error: Some(on_error),
            suspend_mode: None,
            mutate: HookMutationConfig::default(),
            extra: HashMap::new(),
        }
    }

    fn hooks(events: Vec<(HookPhaseEvent, HookSpec)>) -> HooksConfig {
        let mut config = HooksConfig {
            enabled: true,
            ..HooksConfig::default()
        };
        for (event, spec) in events {
            config.events.entry(event).or_default().push(spec);
        }
        config
    }

    #[test]
    fn supported_post_loop_hooks_map_to_engine_classes() {
        let complete = hooks(vec![(
            HookPhaseEvent::PostLoopComplete,
            spec("done", HookOnError::Warn),
        )]);
        assert_eq!(validate_engine_hooks(&complete), Ok(()));
        assert_eq!(notify_classes(&complete).as_deref(), Some("completed"));

        let both = hooks(vec![
            (
                HookPhaseEvent::PostLoopComplete,
                spec("done", HookOnError::Warn),
            ),
            (
                HookPhaseEvent::PostLoopError,
                spec("err", HookOnError::Warn),
            ),
        ]);
        assert_eq!(
            notify_classes(&both).as_deref(),
            Some("completed,failed,stopped")
        );
    }

    #[test]
    fn no_hooks_means_no_notify_block() {
        assert_eq!(notify_classes(&HooksConfig::default()), None);
        let mut disabled = hooks(vec![(
            HookPhaseEvent::PostLoopComplete,
            spec("done", HookOnError::Warn),
        )]);
        disabled.enabled = false;
        assert_eq!(notify_classes(&disabled), None);
        assert_eq!(validate_engine_hooks(&disabled), Ok(()));
    }

    #[test]
    fn every_unsupported_event_refuses_and_names_itself() {
        for event in [
            HookPhaseEvent::PreLoopStart,
            HookPhaseEvent::PostLoopStart,
            HookPhaseEvent::PreIterationStart,
            HookPhaseEvent::PostIterationStart,
            HookPhaseEvent::PrePlanCreated,
            HookPhaseEvent::PostPlanCreated,
            HookPhaseEvent::PreHumanInteract,
            HookPhaseEvent::PostHumanInteract,
            HookPhaseEvent::PreLoopComplete,
            HookPhaseEvent::PreLoopError,
        ] {
            let config = hooks(vec![(event, spec("h", HookOnError::Warn))]);
            let error = validate_engine_hooks(&config).unwrap_err();
            assert_eq!(
                error,
                EngineHookError::UnsupportedEvent {
                    event: event.as_str()
                }
            );
            assert!(error.to_string().contains(event.as_str()), "{error}");
        }
    }

    #[test]
    fn mutating_and_blocking_hooks_refuse_and_name_the_hook() {
        let mut mutating = spec("rewrite", HookOnError::Warn);
        mutating.mutate.enabled = true;
        let error =
            validate_engine_hooks(&hooks(vec![(HookPhaseEvent::PostLoopComplete, mutating)]))
                .unwrap_err();
        assert!(error.to_string().contains("`rewrite`"), "{error}");

        for (on_error, label) in [
            (HookOnError::Block, "block"),
            (HookOnError::Suspend, "suspend"),
        ] {
            let error = validate_engine_hooks(&hooks(vec![(
                HookPhaseEvent::PostLoopError,
                spec("gate", on_error),
            )]))
            .unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains("`gate`") && message.contains(label),
                "{message}"
            );
        }
    }

    #[test]
    fn stop_reasons_map_like_the_engine_classifier() {
        for reason in ["completed", "completion_event", "completion_promise"] {
            assert_eq!(
                phase_event_for_stop_reason(reason),
                HookPhaseEvent::PostLoopComplete
            );
        }
        for reason in ["backend_failed", "max_iterations", "review_unknown"] {
            assert_eq!(
                phase_event_for_stop_reason(reason),
                HookPhaseEvent::PostLoopError
            );
        }
    }

    struct RecordingExecutor(Mutex<Vec<HookRunRequest>>);

    impl HookExecutorContract for RecordingExecutor {
        fn run(
            &self,
            request: HookRunRequest,
        ) -> Result<HookRunResult, super::super::executor::HookExecutorError> {
            self.0.lock().unwrap().push(request);
            Ok(HookRunResult {
                started_at: chrono::Utc::now(),
                ended_at: chrono::Utc::now(),
                duration_ms: 1,
                exit_code: Some(0),
                timed_out: false,
                stdout: super::super::executor::HookStreamOutput::default(),
                stderr: super::super::executor::HookStreamOutput::default(),
            })
        }
    }

    #[test]
    fn dispatch_sends_ralphs_payload_for_the_mapped_event() {
        let snapshot = NotifySnapshot {
            loop_id: "primary".to_string(),
            is_primary: true,
            workspace: PathBuf::from("/work"),
            repo_root: PathBuf::from("/work"),
            max_iterations: 20,
            hooks: hooks(vec![
                (
                    HookPhaseEvent::PostLoopComplete,
                    spec("done", HookOnError::Warn),
                ),
                (
                    HookPhaseEvent::PostLoopError,
                    spec("err", HookOnError::Warn),
                ),
            ]),
        };
        let executor = RecordingExecutor(Mutex::new(Vec::new()));
        let finish = EngineFinishPayload {
            run_id: "run-1".to_string(),
            stop_reason: "max_iterations".to_string(),
            iterations: 20,
        };

        let (event, outcomes) = dispatch(&snapshot, &finish, &executor);

        assert_eq!(event, HookPhaseEvent::PostLoopError);
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].succeeded());
        let requests = executor.0.lock().unwrap();
        assert_eq!(requests[0].hook_name, "err");
        let payload = &requests[0].stdin_payload;
        assert_eq!(payload["phase_event"], "post.loop.error");
        assert_eq!(payload["loop"]["id"], "primary");
        assert_eq!(payload["iteration"]["current"], 20);
        assert_eq!(payload["iteration"]["max"], 20);
        assert_eq!(payload["context"]["termination_reason"], "max_iterations");
    }

    #[test]
    fn timeout_covers_the_slowest_event_plus_margin() {
        let snapshot = NotifySnapshot {
            loop_id: "primary".to_string(),
            is_primary: true,
            workspace: PathBuf::from("/work"),
            repo_root: PathBuf::from("/work"),
            max_iterations: 1,
            hooks: hooks(vec![
                (
                    HookPhaseEvent::PostLoopComplete,
                    spec("a", HookOnError::Warn),
                ),
                (
                    HookPhaseEvent::PostLoopComplete,
                    spec("b", HookOnError::Warn),
                ),
                (HookPhaseEvent::PostLoopError, spec("c", HookOnError::Warn)),
            ]),
        };
        assert_eq!(snapshot.timeout_ms(), 45_000);
    }
}
