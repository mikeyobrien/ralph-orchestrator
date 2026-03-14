use crate::config::{
    HookDefaults, HookMutationConfig, HookOnError, HookPhaseEvent, HookSpec, HookSuspendMode,
    HooksConfig,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;

const HOOK_PAYLOAD_SCHEMA_VERSION: u32 = 1;
const DEFAULT_ACTIVE_HAT: &str = "ralph";

/// Resolves configured hooks for a lifecycle phase-event.
#[derive(Debug, Clone)]
pub struct HookEngine {
    defaults: HookDefaults,
    hooks_by_phase_event: HashMap<HookPhaseEvent, Vec<HookSpec>>,
}

impl HookEngine {
    /// Creates a hook engine from validated hook configuration.
    #[must_use]
    pub fn new(config: &HooksConfig) -> Self {
        Self {
            defaults: config.defaults.clone(),
            hooks_by_phase_event: config.events.clone(),
        }
    }

    /// Resolves hooks for a canonical phase-event key in declaration order.
    #[must_use]
    pub fn resolve_phase_event(&self, phase_event: HookPhaseEvent) -> Vec<ResolvedHookSpec> {
        self.hooks_by_phase_event
            .get(&phase_event)
            .map(|hooks| {
                hooks
                    .iter()
                    .enumerate()
                    .map(|(declaration_order, hook)| {
                        ResolvedHookSpec::from_spec(
                            phase_event,
                            declaration_order,
                            &self.defaults,
                            hook,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolves hooks by phase-event string key.
    ///
    /// Unknown phase-event keys return an empty list.
    #[must_use]
    pub fn resolve_phase_event_str(&self, phase_event: &str) -> Vec<ResolvedHookSpec> {
        HookPhaseEvent::parse(phase_event)
            .map(|phase| self.resolve_phase_event(phase))
            .unwrap_or_default()
    }

    /// Builds the lifecycle JSON payload sent to hook stdin.
    #[must_use]
    pub fn build_payload(
        &self,
        phase_event: HookPhaseEvent,
        input: HookPayloadBuilderInput,
    ) -> HookInvocationPayload {
        self.build_payload_with_timestamp(phase_event, input, Utc::now())
    }

    /// Builds the lifecycle JSON payload sent to hook stdin with a fixed timestamp.
    #[must_use]
    pub fn build_payload_with_timestamp(
        &self,
        phase_event: HookPhaseEvent,
        input: HookPayloadBuilderInput,
        timestamp: DateTime<Utc>,
    ) -> HookInvocationPayload {
        let (phase, event) = split_phase_event(phase_event);
        let HookPayloadBuilderInput {
            loop_id,
            is_primary,
            workspace,
            repo_root,
            pid,
            iteration_current,
            iteration_max,
            context,
        } = input;

        let HookPayloadContextInput {
            active_hat,
            selected_hat,
            selected_task,
            termination_reason,
            human_interact,
            metadata,
        } = context;

        HookInvocationPayload {
            schema_version: HOOK_PAYLOAD_SCHEMA_VERSION,
            phase: phase.to_string(),
            event: event.to_string(),
            phase_event: phase_event.as_str().to_string(),
            timestamp,
            loop_context: HookPayloadLoop {
                id: loop_id,
                is_primary,
                workspace: workspace.to_string_lossy().into_owned(),
                repo_root: repo_root.to_string_lossy().into_owned(),
                pid,
            },
            iteration: HookPayloadIteration {
                current: iteration_current,
                max: iteration_max,
            },
            context: HookPayloadContext {
                active_hat: active_hat.unwrap_or_else(|| DEFAULT_ACTIVE_HAT.to_string()),
                selected_hat,
                selected_task,
                termination_reason,
                human_interact,
            },
            metadata: HookPayloadMetadata {
                accumulated: metadata,
            },
        }
    }
}

fn split_phase_event(phase_event: HookPhaseEvent) -> (&'static str, &'static str) {
    phase_event.as_str().split_once('.').expect(
        "HookPhaseEvent canonical keys always contain a phase prefix and event suffix separated by '.'",
    )
}

/// Hook spec with defaults materialized for runtime dispatch.
#[derive(Debug, Clone)]
pub struct ResolvedHookSpec {
    pub phase_event: HookPhaseEvent,
    pub declaration_order: usize,
    pub name: String,
    pub command: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub timeout_seconds: u64,
    pub max_output_bytes: u64,
    pub on_error: HookOnError,
    pub suspend_mode: HookSuspendMode,
    pub mutate: HookMutationConfig,
}

impl ResolvedHookSpec {
    fn from_spec(
        phase_event: HookPhaseEvent,
        declaration_order: usize,
        defaults: &HookDefaults,
        spec: &HookSpec,
    ) -> Self {
        Self {
            phase_event,
            declaration_order,
            name: spec.name.clone(),
            command: spec.command.clone(),
            cwd: spec.cwd.clone(),
            env: spec.env.clone(),
            timeout_seconds: spec.timeout_seconds.unwrap_or(defaults.timeout_seconds),
            max_output_bytes: spec.max_output_bytes.unwrap_or(defaults.max_output_bytes),
            on_error: spec.on_error.unwrap_or(HookOnError::Warn),
            suspend_mode: spec.suspend_mode.unwrap_or(defaults.suspend_mode),
            mutate: spec.mutate.clone(),
        }
    }
}

/// Input contract for building hook invocation stdin payloads.
#[derive(Debug, Clone)]
pub struct HookPayloadBuilderInput {
    pub loop_id: String,
    pub is_primary: bool,
    pub workspace: PathBuf,
    pub repo_root: PathBuf,
    pub pid: u32,
    pub iteration_current: u32,
    pub iteration_max: u32,
    pub context: HookPayloadContextInput,
}

/// Mutable lifecycle context fields carried in hook stdin payloads.
#[derive(Debug, Clone, Default)]
pub struct HookPayloadContextInput {
    pub active_hat: Option<String>,
    pub selected_hat: Option<String>,
    pub selected_task: Option<String>,
    pub termination_reason: Option<String>,
    pub human_interact: Option<Value>,
    pub metadata: Map<String, Value>,
}

/// Structured lifecycle payload sent to hook stdin as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookInvocationPayload {
    pub schema_version: u32,
    pub phase: String,
    pub event: String,
    pub phase_event: String,
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "loop")]
    pub loop_context: HookPayloadLoop,
    pub iteration: HookPayloadIteration,
    pub context: HookPayloadContext,
    pub metadata: HookPayloadMetadata,
}

/// Loop metadata payload block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPayloadLoop {
    pub id: String,
    pub is_primary: bool,
    pub workspace: String,
    pub repo_root: String,
    pub pid: u32,
}

/// Iteration metadata payload block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPayloadIteration {
    pub current: u32,
    pub max: u32,
}

/// Lifecycle context payload block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPayloadContext {
    pub active_hat: String,
    pub selected_hat: Option<String>,
    pub selected_task: Option<String>,
    pub termination_reason: Option<String>,
    pub human_interact: Option<Value>,
}

/// Mutable metadata payload block.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookPayloadMetadata {
    #[serde(default)]
    pub accumulated: Map<String, Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn hook_spec(name: &str) -> HookSpec {
        HookSpec {
            name: name.to_string(),
            command: vec!["echo".to_string(), name.to_string()],
            cwd: None,
            env: HashMap::new(),
            timeout_seconds: None,
            max_output_bytes: None,
            on_error: Some(HookOnError::Warn),
            suspend_mode: None,
            mutate: HookMutationConfig::default(),
            extra: HashMap::new(),
        }
    }

    fn hooks_config(events: HashMap<HookPhaseEvent, Vec<HookSpec>>) -> HooksConfig {
        HooksConfig {
            enabled: true,
            defaults: HookDefaults {
                timeout_seconds: 45,
                max_output_bytes: 16_384,
                suspend_mode: HookSuspendMode::WaitThenRetry,
            },
            events,
            extra: HashMap::new(),
        }
    }

    fn fixed_time(hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 28, hour, minute, second)
            .single()
            .expect("fixed timestamp")
    }

    fn payload_input() -> HookPayloadBuilderInput {
        HookPayloadBuilderInput {
            loop_id: "loop-1234-abcd".to_string(),
            is_primary: false,
            workspace: PathBuf::from("/repo/.worktrees/loop-1234-abcd"),
            repo_root: PathBuf::from("/repo"),
            pid: 12345,
            iteration_current: 7,
            iteration_max: 100,
            context: HookPayloadContextInput::default(),
        }
    }

    #[test]
    fn resolve_phase_event_preserves_declaration_order() {
        let mut events = HashMap::new();
        events.insert(
            HookPhaseEvent::PreLoopStart,
            vec![
                hook_spec("env-guard"),
                hook_spec("workspace-check"),
                hook_spec("notify"),
            ],
        );
        events.insert(HookPhaseEvent::PostLoopStart, vec![hook_spec("post-loop")]);

        let engine = HookEngine::new(&hooks_config(events));
        let resolved = engine.resolve_phase_event(HookPhaseEvent::PreLoopStart);

        assert_eq!(resolved.len(), 3);
        assert_eq!(resolved[0].name, "env-guard");
        assert_eq!(resolved[0].declaration_order, 0);
        assert_eq!(resolved[1].name, "workspace-check");
        assert_eq!(resolved[1].declaration_order, 1);
        assert_eq!(resolved[2].name, "notify");
        assert_eq!(resolved[2].declaration_order, 2);
        assert!(
            resolved
                .iter()
                .all(|hook| hook.phase_event == HookPhaseEvent::PreLoopStart)
        );
    }

    #[test]
    fn resolve_phase_event_applies_defaults_and_per_hook_overrides() {
        let mut hook_with_overrides = hook_spec("manual-gate");
        hook_with_overrides.timeout_seconds = Some(9);
        hook_with_overrides.max_output_bytes = Some(777);
        hook_with_overrides.on_error = Some(HookOnError::Suspend);
        hook_with_overrides.suspend_mode = Some(HookSuspendMode::RetryBackoff);

        let mut events = HashMap::new();
        events.insert(
            HookPhaseEvent::PreIterationStart,
            vec![hook_spec("defaulted"), hook_with_overrides],
        );

        let engine = HookEngine::new(&hooks_config(events));
        let resolved = engine.resolve_phase_event(HookPhaseEvent::PreIterationStart);

        assert_eq!(resolved.len(), 2);

        assert_eq!(resolved[0].timeout_seconds, 45);
        assert_eq!(resolved[0].max_output_bytes, 16_384);
        assert_eq!(resolved[0].on_error, HookOnError::Warn);
        assert_eq!(resolved[0].suspend_mode, HookSuspendMode::WaitThenRetry);

        assert_eq!(resolved[1].timeout_seconds, 9);
        assert_eq!(resolved[1].max_output_bytes, 777);
        assert_eq!(resolved[1].on_error, HookOnError::Suspend);
        assert_eq!(resolved[1].suspend_mode, HookSuspendMode::RetryBackoff);
    }

    #[test]
    fn resolve_phase_event_returns_empty_for_unconfigured_or_unknown_phase() {
        let mut events = HashMap::new();
        events.insert(HookPhaseEvent::PreLoopStart, vec![hook_spec("env-guard")]);

        let engine = HookEngine::new(&hooks_config(events));

        let missing = engine.resolve_phase_event(HookPhaseEvent::PostIterationStart);
        assert!(missing.is_empty());

        let unknown = engine.resolve_phase_event_str("post.nonexistent.event");
        assert!(unknown.is_empty());
    }

    #[test]
    fn build_payload_maps_loop_iteration_and_context_fields() {
        let engine = HookEngine::new(&hooks_config(HashMap::new()));
        let mut input = payload_input();

        let mut metadata = Map::new();
        metadata.insert("risk_score".to_string(), json!(0.72));

        input.context = HookPayloadContextInput {
            active_hat: Some("ralph".to_string()),
            selected_hat: Some("builder".to_string()),
            selected_task: Some("task-1772314313-a244".to_string()),
            termination_reason: None,
            human_interact: Some(json!({"question": "Proceed?"})),
            metadata,
        };

        let payload = engine.build_payload_with_timestamp(
            HookPhaseEvent::PostIterationStart,
            input,
            fixed_time(21, 47, 0),
        );

        assert_eq!(payload.schema_version, HOOK_PAYLOAD_SCHEMA_VERSION);
        assert_eq!(payload.phase, "post");
        assert_eq!(payload.event, "iteration.start");
        assert_eq!(payload.phase_event, "post.iteration.start");
        assert_eq!(payload.loop_context.id, "loop-1234-abcd");
        assert!(!payload.loop_context.is_primary);
        assert_eq!(
            payload.loop_context.workspace,
            "/repo/.worktrees/loop-1234-abcd"
        );
        assert_eq!(payload.loop_context.repo_root, "/repo");
        assert_eq!(payload.loop_context.pid, 12345);
        assert_eq!(payload.iteration.current, 7);
        assert_eq!(payload.iteration.max, 100);
        assert_eq!(payload.context.active_hat, "ralph");
        assert_eq!(payload.context.selected_hat.as_deref(), Some("builder"));
        assert_eq!(
            payload.context.selected_task.as_deref(),
            Some("task-1772314313-a244")
        );
        assert_eq!(payload.metadata.accumulated["risk_score"], json!(0.72));

        let value = serde_json::to_value(&payload).expect("serialize payload");
        assert_eq!(value["loop"]["id"], "loop-1234-abcd");
        assert_eq!(value["context"]["selected_hat"], "builder");
        assert_eq!(value["context"]["selected_task"], "task-1772314313-a244");
        assert_eq!(value["metadata"]["accumulated"]["risk_score"], json!(0.72));
    }

    #[test]
    fn build_payload_defaults_optional_context_fields() {
        let engine = HookEngine::new(&hooks_config(HashMap::new()));
        let payload = engine.build_payload_with_timestamp(
            HookPhaseEvent::PreLoopStart,
            payload_input(),
            fixed_time(21, 48, 0),
        );

        assert_eq!(payload.phase, "pre");
        assert_eq!(payload.event, "loop.start");
        assert_eq!(payload.phase_event, "pre.loop.start");
        assert_eq!(payload.context.active_hat, DEFAULT_ACTIVE_HAT);
        assert!(payload.context.selected_hat.is_none());
        assert!(payload.context.selected_task.is_none());
        assert!(payload.context.termination_reason.is_none());
        assert!(payload.context.human_interact.is_none());
        assert!(payload.metadata.accumulated.is_empty());

        let value = serde_json::to_value(&payload).expect("serialize payload");
        assert!(value["context"]["selected_hat"].is_null());
        assert!(value["context"]["selected_task"].is_null());
        assert!(
            value["metadata"]["accumulated"]
                .as_object()
                .expect("accumulated metadata object")
                .is_empty()
        );
    }
}
