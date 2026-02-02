use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

pub(crate) fn test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
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
}

impl CwdGuard {
    pub(crate) fn set(path: &Path) -> Self {
        let lock = test_lock();
        let original = safe_current_dir();
        std::env::set_current_dir(path).expect("set current dir");
        Self {
            _lock: lock,
            original,
        }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}
