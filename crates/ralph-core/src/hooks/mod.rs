//! Lifecycle hook runtime contracts and orchestration primitives.

mod engine;
mod executor;
mod suspend_state;

pub use crate::config::{
    HookDefaults, HookMutationConfig, HookOnError, HookPhaseEvent, HookSpec, HookSuspendMode,
    HooksConfig,
};
pub use engine::{
    HookEngine, HookInvocationPayload, HookPayloadBuilderInput, HookPayloadContext,
    HookPayloadContextInput, HookPayloadIteration, HookPayloadLoop, HookPayloadMetadata,
    ResolvedHookSpec,
};
#[cfg(unix)]
pub use executor::signal_active_hook_process_groups;
pub use executor::{
    HookExecutor, HookExecutorContract, HookExecutorError, HookRunRequest, HookRunResult,
    HookStreamOutput,
};
pub use suspend_state::{
    SUSPEND_STATE_SCHEMA_VERSION, SuspendLifecycleState, SuspendStateRecord, SuspendStateStore,
    SuspendStateStoreError,
};
