use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

pub(crate) fn test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn workspace_root_env_override_slot() -> &'static Mutex<Option<WorkspaceRootEnvOverride>> {
    static OVERRIDE: OnceLock<Mutex<Option<WorkspaceRootEnvOverride>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceRootEnvOverride {
    Unset,
    Set(PathBuf),
}

pub(crate) fn workspace_root_env_override() -> Option<WorkspaceRootEnvOverride> {
    workspace_root_env_override_slot()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clone()
}

pub(crate) fn safe_current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| {
        let fallback = std::env::temp_dir();
        std::env::set_current_dir(&fallback).expect("set fallback cwd");
        fallback
    })
}

pub(crate) struct CwdGuard {
    _lock: MutexGuard<'static, ()>,
    original: PathBuf,
    workspace_root_override_previous: Option<WorkspaceRootEnvOverride>,
    workspace_root_override_active: bool,
}

impl CwdGuard {
    pub(crate) fn set(path: &Path) -> Self {
        Self::set_internal(path, None)
    }

    pub(crate) fn set_ignoring_workspace_root_env(path: &Path) -> Self {
        Self::set_internal(path, Some(WorkspaceRootEnvOverride::Unset))
    }

    pub(crate) fn set_with_workspace_root_env(path: &Path, workspace_root: PathBuf) -> Self {
        Self::set_internal(path, Some(WorkspaceRootEnvOverride::Set(workspace_root)))
    }

    fn set_internal(
        path: &Path,
        workspace_root_override: Option<WorkspaceRootEnvOverride>,
    ) -> Self {
        let lock = test_lock();
        let original = safe_current_dir();
        std::env::set_current_dir(path).expect("set current dir");

        let previous = if let Some(override_value) = workspace_root_override.as_ref() {
            let mut slot = workspace_root_env_override_slot()
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            let previous = slot.clone();
            *slot = Some(override_value.clone());
            previous
        } else {
            None
        };

        Self {
            _lock: lock,
            original,
            workspace_root_override_previous: previous,
            workspace_root_override_active: workspace_root_override.is_some(),
        }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        if self.workspace_root_override_active {
            let mut slot = workspace_root_env_override_slot()
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            *slot = self.workspace_root_override_previous.clone();
        }

        let _ = std::env::set_current_dir(&self.original);
    }
}
